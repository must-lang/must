use std::collections::HashMap;

use salsa::Database;

use crate::{
    bytecode::{ir, place::Place, value::Value},
    layout::get_size,
    parser::ast,
    typecheck::{self, SType},
};

pub struct LowerCtx<'a> {
    pub db: &'a dyn Database,
    pub scopes: Vec<HashMap<ast::Ident<'a>, Value>>,
    pub builder: &'a mut ir::IrBuilder,
    pub types: &'a HashMap<ast::ExprId<'a>, typecheck::SType>,
}

impl<'a> LowerCtx<'a> {
    pub fn lower_value(&mut self, expr: ast::ExprId<'a>, dest: Option<Place>) -> Value {
        let size = get_size(self.types.get(&expr).unwrap());
        match expr.data(self.db) {
            ast::ExprData::Num(n) => {
                let v = Value::Int(n);
                if let Some(dest) = dest {
                    v.write_to(dest, 1, self.builder);
                }
                v
            }
            ast::ExprData::FnCall(name, args) => {
                let vals: Vec<_> = args
                    .iter()
                    .map(|e| self.lower_value(*e, None).load_scalar(self.builder))
                    // Unlike builtin NO rev here.
                    .collect();
                let reg = self.builder.new_reg();
                self.builder
                    .push_instr(ir::Inst::FnCall(reg, name.text(self.db).clone(), vals));
                let v = Value::LVal(Place::Reg(reg));
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                };
                v
            }
            ast::ExprData::Error => panic!("cannot lower code with errors"),
            x @ (ast::ExprData::True | ast::ExprData::False) => {
                let reg = self.builder.new_reg();
                self.builder
                    .push_instr(ir::Inst::LoadBool(reg, x == ast::ExprData::True));
                let v = Value::LVal(Place::Reg(reg));
                if let Some(dest) = dest {
                    v.write_to(dest, 1, self.builder);
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
            ast::ExprData::Assign(e1, e2) => {
                let v1 = self.lower_value(e1, None);

                match v1 {
                    Value::LVal(place) => {
                        self.lower_value(e2, Some(place));
                    }
                    Value::Unit => (),
                    Value::Int(_) => panic!(),
                }

                Value::Unit
            }
            ast::ExprData::AddressOf(expr) => {
                let reg = match self.lower_value(expr, None) {
                    Value::Unit => todo!(),
                    Value::Int(_) => todo!(),
                    Value::LVal(place) => place.as_addr(self.builder),
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
            ast::ExprData::Index(expr, id_expr) => {
                let id = self.lower_value(id_expr, None).load_scalar(self.builder);
                let size_reg = self.builder.new_reg();
                self.builder.push_instr(ir::Inst::LoadInt(size_reg, size));
                let offset_reg = self.builder.new_reg();
                self.builder
                    .push_instr(ir::Inst::Mul(offset_reg, size_reg, id));
                let base_ptr = match self.lower_value(expr, None) {
                    Value::Unit => todo!(),
                    Value::Int(_) => todo!(),
                    Value::LVal(place) => place.as_addr(self.builder),
                };
                self.builder
                    .push_instr(ir::Inst::Add(base_ptr, base_ptr, offset_reg));
                let v = Value::LVal(Place::DynamicPtr {
                    base: base_ptr,
                    offset: 0,
                });
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

                for (pat, arm_expr) in arms {
                    // TODO: There is no need to create a new block if pattern is irrefutable
                    let next_block = self.builder.new_block();

                    self.lower_pat(
                        pat,
                        val,
                        Some(next_block),
                        self.types.get(&target_expr).unwrap(),
                    );

                    self.lower_value(arm_expr, Some(place));

                    self.builder.finish_block(ir::Terminator::Jump(end_block));

                    self.builder.switch_to_block(next_block);
                }

                self.builder.switch_to_block(end_block);
                Value::LVal(place)
            }
            ast::ExprData::Seq(e1, e2) => {
                self.lower_value(e1, None);
                self.lower_value(e2, dest)
            }
            ast::ExprData::BinOp(op, e1, e2) => {
                let v1 = self.lower_value(e1, None).load_scalar(self.builder);
                let v2 = self.lower_value(e2, None).load_scalar(self.builder);
                let reg = self.builder.new_reg();
                let inst = match op {
                    ast::Op::Add => ir::Inst::Add(reg, v1, v2),
                    ast::Op::Sub => ir::Inst::Sub(reg, v1, v2),
                    ast::Op::Mul => ir::Inst::Mul(reg, v1, v2),
                    ast::Op::Div => todo!(),
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
            x @ (ast::PatternData::True | ast::PatternData::False) => {
                let t_reg = self.builder.new_reg();

                self.builder
                    .push_instr(ir::Inst::LoadBool(t_reg, x == ast::PatternData::True));

                let cond = self.builder.new_reg();
                let reg = s.load_scalar(self.builder);
                self.builder.push_instr(ir::Inst::CmpEq(cond, reg, t_reg));

                let success = self.builder.new_block();

                self.builder.finish_block(ir::Terminator::BranchIf {
                    cond,
                    th: success,
                    el: fail_block.unwrap(),
                });

                self.builder.switch_to_block(success);
            }
            ast::PatternData::Tuple(pats) => match s {
                Value::LVal(Place::Stack { slot, mut offset }) => {
                    let tps = match tp {
                        SType::Tuple(tps) => tps,
                        _ => panic!("{:?}", tp),
                    };
                    for (pat, tp) in pats.into_iter().zip(tps) {
                        self.lower_pat(
                            pat,
                            Value::LVal(Place::Stack { slot, offset }),
                            fail_block,
                            tp,
                        );
                        offset += get_size(tp);
                    }
                }
                _ => todo!(),
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

    fn lookup(&self, ident: ast::Ident<'_>) -> Value {
        for scope in self.scopes.iter().rev() {
            if let Some(tp) = scope.get(&ident) {
                return *tp;
            }
        }
        panic!()
    }

    pub fn extend(&mut self, x: ast::Ident<'a>, storage: Value) {
        self.scopes.last_mut().unwrap().insert(x, storage);
    }
}
