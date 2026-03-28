use std::collections::HashMap;

use salsa::Database;

use crate::{
    layout::get_size,
    parser::ast,
    typecheck::{self, SType},
    vm::ir,
};

#[salsa::tracked]
pub fn compile<'db>(db: &'db dyn Database, prog: ast::File<'db>) -> ir::Prog {
    let types = typecheck::check_file(db, prog).types;
    let mut funcs = HashMap::new();
    for def in prog.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                let (name, func) = lower_function(db, *fn_def, &types);
                funcs.insert(name, func);
            }
        }
    }
    ir::Prog { funcs }
}

pub fn lower_function<'db>(
    db: &'db dyn Database,
    ast_fn: ast::FnDef<'db>,
    types: &'db HashMap<ast::ExprId<'db>, typecheck::SType>,
) -> (String, ir::Func) {
    let mut builder = ir::IrBuilder::new();

    let mut ctx = LowerCtx {
        db,
        scopes: vec![HashMap::new()],
        builder: &mut builder,
        types,
    };

    for (pat, _) in ast_fn.args(db) {
        let reg = ctx.builder.new_reg();
        ctx.lower_pat(pat, Value::LVal(Place::Reg(reg)), None, &SType::Error);
    }

    let res_reg = ctx.builder.new_reg();
    ctx.lower_value(ast_fn.body(db), Some(Place::Reg(res_reg)));

    builder.blocks[builder.current_block.0].terminator = ir::Terminator::Return(res_reg);

    (
        ast_fn.name(db).text(db).clone(),
        ir::Func {
            register_count: builder.next_reg,
            blocks: builder.blocks,
            stack_slots: builder.stack_slots,
        },
    )
}

struct LowerCtx<'a> {
    db: &'a dyn Database,
    scopes: Vec<HashMap<ast::Ident<'a>, Value>>,
    builder: &'a mut ir::IrBuilder,
    types: &'a HashMap<ast::ExprId<'a>, typecheck::SType>,
}

#[derive(Debug, Clone, Copy)]
enum Place {
    Reg(ir::Reg),
    DynamicPtr {
        base: ir::Reg,
        offset: usize,
    },
    Stack {
        slot: ir::StackSlotId,
        offset: usize,
    },
}

#[derive(Debug, Clone, Copy)]
enum Value {
    Unit,
    Int(usize),
    LVal(Place),
}

impl Place {
    pub fn add_offset(self, n: usize) -> Self {
        match self {
            Place::DynamicPtr { base, offset } => Place::DynamicPtr {
                base,
                offset: offset + n,
            },
            Place::Stack { slot, offset } => Place::Stack {
                slot,
                offset: offset + n,
            },
            Place::Reg(reg) => panic!(),
        }
    }

    fn as_addr(&self, builder: &mut ir::IrBuilder) -> ir::Reg {
        match self {
            Place::Reg(reg) => panic!(),
            Place::DynamicPtr { base, offset } => {
                let reg = builder.new_reg();
                builder.push_instr(ir::Inst::AddImm(reg, *base, *offset));
                reg
            }
            Place::Stack { slot, offset } => {
                let reg = builder.new_reg();
                builder.push_instr(ir::Inst::StackAddr(reg, *slot, *offset));
                reg
            }
        }
    }
}

impl Value {
    pub fn load_scalar(self, builder: &mut ir::IrBuilder) -> ir::Reg {
        match self {
            Value::LVal(place) => builder.load_from_place(place),
            Value::Unit => panic!(),
            Value::Int(n) => {
                let reg = builder.new_reg();
                builder.push_instr(ir::Inst::LoadInt(reg, n));
                reg
            }
        }
    }

    pub fn write_to(self, dest: Place, size: usize, builder: &mut ir::IrBuilder) {
        match self {
            Value::LVal(src) => match (src, dest) {
                (Place::Reg(reg), _) => builder.store_to_place(dest, reg),
                (_, Place::Reg(_)) => {
                    let reg = builder.load_from_place(src);
                    builder.store_to_place(dest, reg)
                }
                (_, _) => {
                    let r1 = src.as_addr(builder);
                    let r2 = dest.as_addr(builder);
                    builder.push_instr(ir::Inst::MemCopy {
                        src: r1,
                        dst: r2,
                        len: size,
                    });
                }
            },
            Value::Unit => (),
            Value::Int(n) => {
                let reg = self.load_scalar(builder);
                builder.store_to_place(dest, reg);
            }
        }
    }
}

impl ir::IrBuilder {
    pub fn load_from_place(&mut self, place: Place) -> ir::Reg {
        match place {
            Place::DynamicPtr { base, offset } => {
                let reg = self.new_reg();
                self.push_instr(ir::Inst::Load(reg, base, offset));
                reg
            }
            Place::Stack { slot, offset } => {
                let reg = self.new_reg();
                self.push_instr(ir::Inst::StackLoad(reg, slot, offset));
                reg
            }
            Place::Reg(reg) => reg,
        }
    }

    pub fn store_to_place(&mut self, dest: Place, reg: ir::Reg) {
        match dest {
            Place::DynamicPtr { base, offset } => {
                self.push_instr(ir::Inst::Store(base, offset, reg))
            }
            Place::Stack { slot, offset } => {
                self.push_instr(ir::Inst::StackStore(slot, offset, reg))
            }
            Place::Reg(reg_dest) => self.push_instr(ir::Inst::Assign(reg_dest, reg)),
        }
    }
}

impl<'a> LowerCtx<'a> {
    fn lower_value(&mut self, expr: ast::ExprId<'a>, dest: Option<Place>) -> Value {
        let size = get_size(self.types.get(&expr).unwrap());
        match expr.data(self.db) {
            ast::ExprData::Num(n) => {
                let v = Value::Int(n);
                if let Some(dest) = dest {
                    v.write_to(dest, 1, self.builder);
                }
                v
            }
            ast::ExprData::Builtin(name, args) => {
                let mut vals: Vec<_> = args
                    .iter()
                    .map(|e| self.lower_value(*e, None))
                    .rev() // why rev ??????
                    .collect();

                let v = match name.text(self.db).as_str() {
                    "intadd" => {
                        let x = vals.pop().unwrap().load_scalar(self.builder);
                        let y = vals.pop().unwrap().load_scalar(self.builder);
                        let reg = self.builder.new_reg();
                        self.builder.push_instr(ir::Inst::Add(reg, x, y));
                        Value::LVal(Place::Reg(reg))
                    }
                    "intle" => {
                        let x = vals.pop().unwrap().load_scalar(self.builder);
                        let y = vals.pop().unwrap().load_scalar(self.builder);
                        let reg = self.builder.new_reg();
                        self.builder.push_instr(ir::Inst::CmpLe(reg, x, y));
                        Value::LVal(Place::Reg(reg))
                    }
                    "printnum" => {
                        let x = vals.pop().unwrap().load_scalar(self.builder);
                        self.builder.push_instr(ir::Inst::PrintNum(x));
                        Value::Unit
                    }
                    _ => panic!("runtime error: unknown builtin function"),
                };
                if let Some(dest) = dest {
                    v.write_to(dest, size, self.builder);
                };
                v
            }
            ast::ExprData::FnCall(name, args) => {
                let vals: Vec<_> = args
                    .iter()
                    .map(|e| self.lower_value(*e, None).load_scalar(self.builder))
                    .rev()
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
                    Value::Int(_) => {
                        let reg = v1.load_scalar(self.builder);
                    }
                }

                Value::Unit
            }
            ast::ExprData::AddressOf(expr_id) => todo!(),
            ast::ExprData::Load(expr_id) => todo!(),
            ast::ExprData::Store(expr_id, expr_id1) => todo!(),
            ast::ExprData::Tuple(exprs) => {
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
            ast::ExprData::Array(expr_ids) => todo!(),
            ast::ExprData::Index(expr_id, expr_id1) => todo!(),
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
        }
    }

    fn lower_pat(
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
            ast::PatternData::Var { name, is_mut } => {
                let s = match s {
                    Value::Unit => s,
                    Value::Int(_) => {
                        if is_mut {
                            let reg = s.load_scalar(self.builder);
                            Value::LVal(Place::Reg(reg))
                        } else {
                            s
                        }
                    }
                    Value::LVal(_) => s,
                };
                self.extend(name, s);
            }
        }
    }

    fn lookup(&self, ident: ast::Ident<'_>) -> Value {
        for scope in self.scopes.iter().rev() {
            if let Some(tp) = scope.get(&ident) {
                return tp.clone();
            }
        }
        panic!()
    }

    pub fn extend(&mut self, x: ast::Ident<'a>, storage: Value) {
        self.scopes.last_mut().unwrap().insert(x, storage);
    }
}
