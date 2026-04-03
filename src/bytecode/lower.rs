use std::collections::HashMap;

use salsa::Database;

use crate::{
    bytecode::{ir, place::Place, value::Value},
    layout::get_size,
    parser::ast,
    typecheck::{self, Coercion, SType},
};

pub struct LowerCtx<'a> {
    pub db: &'a dyn Database,
    pub scopes: Vec<HashMap<ast::Ident<'a>, Value>>,
    pub builder: &'a mut ir::IrBuilder,
    pub types: &'a HashMap<ast::ExprId<'a>, SType>,
    pub coercions: &'a HashMap<ast::ExprId<'a>, Coercion>,
}

impl<'a> LowerCtx<'a> {
    /// Lower expression yielding a value, while applying neccessary coercions.
    ///
    /// This will also attempt to write the value into the destination, if provided.
    pub fn lower_value(&mut self, expr: ast::ExprId<'a>, dest: Option<Place>) -> Value {
        match self.coercions.get(&expr) {
            Some(Coercion::ArrayPtrToSlice) => self.lower_array_ptr_to_slice(expr, dest),
            None => self.lower_value_inner(expr, dest),
        }
    }

    /// Lower expression yielding a value, without applying coercions.
    ///
    /// This function is mostly to be used when generating code for coercions.
    pub fn lower_value_inner(&mut self, expr: ast::ExprId<'a>, dest: Option<Place>) -> Value {
        let tp = self.types.get(&expr).unwrap();
        let size = get_size(tp);
        match expr.data(self.db) {
            ast::ExprData::Error => panic!("cannot lower code with errors"),

            ast::ExprData::Num(n) => {
                let v = Value::Int(n);
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                }
                v
            }

            x @ (ast::ExprData::True | ast::ExprData::False) => {
                let reg = self.builder.new_reg();
                self.builder
                    .push_instr(ir::Inst::LoadBool(reg, x == ast::ExprData::True));
                let v = Value::LVal(Place::Reg(reg));
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                };
                v
            }

            ast::ExprData::Var(ident) => {
                let val = self.lookup(ident);
                if let Some(dest) = dest {
                    val.write_to(dest, size, self.builder);
                }
                val
            }

            ast::ExprData::Let(pat, e1, e2) => {
                let val = self.lower_value(e1, None);
                self.lower_pat(pat, val, None, self.types.get(&e1).unwrap());
                self.lower_value(e2, dest)
            }

            ast::ExprData::Seq(e1, e2) => {
                self.lower_value(e1, None);
                self.lower_value(e2, dest)
            }

            ast::ExprData::FnCall(name, args) => {
                let mut vals: Vec<_> = args
                    .iter()
                    .map(|e| {
                        let arg_size = match self.coercions.get(e) {
                            Some(Coercion::ArrayPtrToSlice) => 2,
                            None => get_size(self.types.get(e).unwrap()),
                        };
                        let val = self.lower_value(*e, None);

                        if arg_size <= 1 {
                            // Scalars fit in a register
                            val.load_scalar(self.builder)
                        } else {
                            // Aggregates must be passed by pointer
                            match val {
                                Value::LVal(place) => place.as_addr(self.builder),
                                _ => panic!("Expected aggregate argument to be an LVal"),
                            }
                        }
                    })
                    .collect();

                let ret_reg = self.builder.new_reg();
                let place = dest.unwrap_or_else(|| {
                    if size <= 1 {
                        Place::Reg(ret_reg)
                    } else {
                        Place::Stack {
                            slot: self.builder.new_stack_slot(size),
                            offset: 0,
                        }
                    }
                });
                if size > 1 {
                    vals.insert(0, place.as_addr(self.builder))
                }
                self.builder.push_instr(ir::Inst::FnCall(
                    ret_reg,
                    name.text(self.db).clone(),
                    vals,
                ));

                if size <= 1 {
                    // Scalar return: the result is physically in `reg`.
                    let v = Value::LVal(Place::Reg(ret_reg));

                    if let Some(dest_place) = dest {
                        v.write_to(dest_place, size, self.builder);
                    }

                    Value::LVal(place)
                } else {
                    Value::LVal(place)
                }
            }

            ast::ExprData::Assign(e1, e2) => {
                let v1 = self.lower_value(e1, None);

                match v1 {
                    Value::LVal(place) => {
                        self.lower_value(e2, Some(place));
                    }
                    Value::Unit => (),
                    Value::Int(_) => panic!("cannot assign to literal"),
                }

                Value::Unit
            }

            ast::ExprData::AddressOf(expr) => {
                let reg = match self.lower_value(expr, None) {
                    Value::LVal(place) => place.as_addr(self.builder),
                    _ => panic!("cannot take address of this expression"),
                };
                let v = Value::LVal(Place::Reg(reg));
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                }
                v
            }

            ast::ExprData::Deref(expr) => {
                let ptr = self.lower_value(expr, None).load_scalar(self.builder);
                let v = Value::LVal(Place::DynamicPtr {
                    base: ptr,
                    offset: 0,
                });
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                }
                v
            }

            ast::ExprData::Array(exprs) | ast::ExprData::Tuple(exprs) => {
                let place = dest.unwrap_or_else(|| Place::Stack {
                    slot: self.builder.new_stack_slot(size),
                    offset: 0,
                });
                let mut offset = 0;
                for e in exprs {
                    self.lower_value(e, Some(place.add_offset(offset)));
                    offset += get_size(self.types.get(&e).unwrap())
                }
                Value::LVal(place)
            }

            ast::ExprData::Index(place_expr, id_expr) => {
                let id = self.lower_value(id_expr, None).load_scalar(self.builder);

                let size_reg = self.builder.new_reg();
                self.builder.push_instr(ir::Inst::LoadInt(size_reg, size));

                let offset_reg = self.builder.new_reg();
                self.builder
                    .push_instr(ir::Inst::Mul(offset_reg, size_reg, id));

                let base = match self.lower_value(place_expr, None) {
                    Value::LVal(place) => match self.types.get(&place_expr).unwrap() {
                        SType::Array(_, _) => place.as_addr(self.builder),
                        // load the first value of the slice which is ptr
                        SType::Slice { .. } => self.builder.load_from_place(place),
                        _ => panic!("cant index {:?}", tp),
                    },
                    _ => panic!("cant index {:?}", tp),
                };

                self.builder
                    .push_instr(ir::Inst::Add(base, base, offset_reg));

                let v = Value::LVal(Place::DynamicPtr { base, offset: 0 });
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                }
                v
            }

            ast::ExprData::Match(target_expr, arms) => {
                let val = self.lower_value(target_expr, None);

                let end_block = self.builder.new_block();

                let place = dest.unwrap_or_else(|| Place::Stack {
                    slot: self.builder.new_stack_slot(size),
                    offset: 0,
                });

                let pat_tp = self.types.get(&target_expr).unwrap();

                for (pat, arm_expr) in arms {
                    // TODO: There is no need to create a new block if pattern is irrefutable
                    let next_block = self.builder.new_block();

                    self.lower_pat(pat, val, Some(next_block), pat_tp);

                    self.lower_value(arm_expr, Some(place));

                    self.builder.finish_block(ir::Terminator::Jump(end_block));

                    self.builder.switch_to_block(next_block);
                }

                self.builder.switch_to_block(end_block);
                Value::LVal(place)
            }

            ast::ExprData::BinOp(op, e1, e2) => {
                let v1 = self.lower_value(e1, None).load_scalar(self.builder);
                let v2 = self.lower_value(e2, None).load_scalar(self.builder);
                let reg = self.builder.new_reg();
                let inst = match op {
                    ast::Op::Add => ir::Inst::Add(reg, v1, v2),
                    ast::Op::Sub => ir::Inst::Sub(reg, v1, v2),
                    ast::Op::Mul => ir::Inst::Mul(reg, v1, v2),
                    ast::Op::Div => todo!("division not implemented yet"),

                    ast::Op::Le => ir::Inst::CmpLe(reg, v1, v2),
                    ast::Op::Eq => ir::Inst::CmpEq(reg, v1, v2),
                };
                self.builder.push_instr(inst);
                let v = Value::LVal(Place::Reg(reg));
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                }
                v
            }
        }
    }

    pub fn lower_pat(
        &mut self,
        pat: ast::PatternId<'a>,
        s: Value,
        fail_block: Option<ir::BlockId>,
        tp: &typecheck::SType,
    ) {
        match pat.data(self.db) {
            ast::PatternData::Wildcard => (),

            ast::PatternData::True => {
                let cond = s.load_scalar(self.builder);

                let success = self.builder.new_block();

                self.builder.finish_block(ir::Terminator::BranchIf {
                    cond,
                    th: success,
                    el: fail_block.unwrap(),
                });

                self.builder.switch_to_block(success);
            }

            ast::PatternData::False => {
                let cond = s.load_scalar(self.builder);

                let success = self.builder.new_block();

                // if cond is true, then we failed
                self.builder.finish_block(ir::Terminator::BranchIf {
                    cond,
                    th: fail_block.unwrap(),
                    el: success,
                });

                self.builder.switch_to_block(success);
            }

            ast::PatternData::Tuple(pats) => match s {
                Value::LVal(place) => {
                    let mut offset = 0;
                    let tps = match tp {
                        SType::Tuple(tps) => tps,
                        _ => panic!("expected tuple, got: {:?}", tp),
                    };
                    for (pat, tp) in pats.into_iter().zip(tps) {
                        self.lower_pat(pat, Value::LVal(place.add_offset(offset)), fail_block, tp);
                        offset += get_size(tp);
                    }
                }
                _ => panic!("{:?} is not a tuple", s),
            },

            ast::PatternData::Var { name, .. } => {
                // let s = match s {
                //     Value::Unit => s,
                //     Value::Int(_) => {
                //         if is_mut {
                //             let reg = s.load_scalar(self.builder);
                //             Value::LVal(Place::Reg(reg))
                //         } else {
                //             s
                //         }
                //     }
                //     Value::LVal(_) => s,
                // };

                // Keep everything on stack for now.
                let size = get_size(tp);
                let place = Place::Stack {
                    slot: self.builder.new_stack_slot(size),
                    offset: 0,
                };
                s.write_to(place, size, self.builder);
                self.extend(name, Value::LVal(place));
            }
        }
    }

    fn lower_array_ptr_to_slice(&mut self, expr: ast::ExprId<'a>, dest: Option<Place>) -> Value {
        // A. Lower the original expression without `dest`.
        // This will give us the thin pointer (*mut [N]int)
        let thin_ptr_reg = self.lower_value_inner(expr, None).load_scalar(self.builder);

        // B. Extract the array length from the typechecker's knowledge
        let arr_type = self.types.get(&expr).unwrap();
        let len = match arr_type {
            SType::Ptr { tp, .. } => match tp.as_ref() {
                SType::Array(len, _) => *len,
                _ => panic!("Expected Array inside Ptr"),
            },
            _ => panic!("Expected Ptr type"),
        };

        // C. Load length into register
        let len_reg = self.builder.new_reg();
        self.builder.push_instr(ir::Inst::LoadInt(len_reg, len));

        // D. Construct the Fat Pointer (Slice) in memory
        let place = dest.unwrap_or_else(|| Place::Stack {
            slot: self.builder.new_stack_slot(2),
            offset: 0,
        });

        self.builder.store_to_place(place, thin_ptr_reg);
        self.builder.store_to_place(place.add_offset(1), len_reg);

        Value::LVal(place)
    }

    fn lookup(&self, ident: ast::Ident<'_>) -> Value {
        for scope in self.scopes.iter().rev() {
            if let Some(tp) = scope.get(&ident) {
                return *tp;
            }
        }
        panic!("{:?} not in scope", ident)
    }

    pub fn extend(&mut self, x: ast::Ident<'a>, storage: Value) {
        self.scopes.last_mut().unwrap().insert(x, storage);
    }
}
