use std::collections::HashMap;

use salsa::Database;

use crate::{parser::ast, vm::ir};

#[salsa::tracked]
pub fn compile<'db>(db: &'db dyn Database, prog: ast::File<'db>) -> ir::Prog {
    let mut funcs = HashMap::new();
    for def in prog.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                let (name, func) = lower_function(db, *fn_def);
                funcs.insert(name, func);
            }
        }
    }
    ir::Prog { funcs }
}

#[salsa::tracked]
pub fn lower_function<'db>(db: &'db dyn Database, ast_fn: ast::FnDef<'db>) -> (String, ir::Func) {
    let mut builder = ir::IrBuilder::new();

    let mut ctx = LowerCtx {
        db,
        scopes: vec![HashMap::new()],
        builder: &mut builder,
    };

    for (pat, _) in ast_fn.args(db) {
        let reg = ctx.builder.new_reg();
        ctx.lower_pat(pat, reg, None)
    }

    let result_reg = ctx.lower_expr(ast_fn.body(db));

    builder.blocks[builder.current_block.0].terminator = ir::Terminator::Return(result_reg);

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
    scopes: Vec<HashMap<ast::Ident<'a>, ir::Reg>>,
    builder: &'a mut ir::IrBuilder,
}

impl<'a> LowerCtx<'a> {
    fn lower_expr(&mut self, expr: ast::ExprId<'a>) -> ir::Reg {
        match expr.data(self.db) {
            ast::ExprData::Num(n) => {
                let reg = self.builder.new_reg();
                self.builder.push_instr(ir::Inst::LoadInt(reg, n));
                reg
            }
            ast::ExprData::Builtin(name, args) => {
                let mut vals: Vec<ir::Reg> =
                    args.iter().map(|e| self.lower_expr(*e)).rev().collect();
                match name.text(self.db).as_str() {
                    "intadd" => {
                        let x = vals.pop().unwrap();
                        let y = vals.pop().unwrap();
                        let reg = self.builder.new_reg();
                        self.builder.push_instr(ir::Inst::Add(reg, x, y));
                        reg
                    }
                    // "printnum" => {
                    //     let x = vals.pop().unwrap();
                    //     match x {
                    //         Some(Value::Num(x)) => println!("output: {x}"),
                    //         _ => panic!("runtime error"),
                    //     }
                    //     Value::Tuple(vec![])
                    // }
                    _ => panic!("runtime error: unknown builtin function"),
                }
            }
            ast::ExprData::FnCall(name, args) => {
                let vals: Vec<ir::Reg> = args.iter().map(|e| self.lower_expr(*e)).rev().collect();
                let reg = self.builder.new_reg();
                self.builder
                    .push_instr(ir::Inst::FnCall(reg, name.text(self.db).clone(), vals));
                reg
            }
            ast::ExprData::Error => panic!("cannot lower code with errors"),
            ast::ExprData::True => {
                let reg = self.builder.new_reg();
                self.builder.push_instr(ir::Inst::LoadBool(reg, true));
                reg
            }
            ast::ExprData::False => {
                let reg = self.builder.new_reg();
                self.builder.push_instr(ir::Inst::LoadBool(reg, false));
                reg
            }
            ast::ExprData::Var(ident) => self.lookup(ident),
            ast::ExprData::Let(pat, e1, e2) => {
                let reg = self.lower_expr(e1);
                self.lower_pat(pat, reg, None);
                self.lower_expr(e2)
            }
            ast::ExprData::Assign(e1, e2) => {
                let r1 = self.lower_expr(e1);
                let r2 = self.lower_expr(e2);
                let reg = self.builder.new_reg();
                // TODO: the actual instruction here depends on the type of assignment
                self.builder.push_instr(ir::Inst::Assign(r1, r2));
                reg
            }
            ast::ExprData::AddressOf(expr_id) => todo!(),
            ast::ExprData::Load(expr_id) => todo!(),
            ast::ExprData::Store(expr_id, expr_id1) => todo!(),
            ast::ExprData::Tuple(exprs) => todo!(),
            ast::ExprData::Array(expr_ids) => todo!(),
            ast::ExprData::Index(expr_id, expr_id1) => todo!(),
            ast::ExprData::Match(expr, arms) => {
                let val_reg = self.lower_expr(expr);
                let res_reg = self.builder.new_reg();
                let end_block = self.builder.new_block();

                for (pat, expr) in arms {
                    let next_block = self.builder.new_block();

                    self.lower_pat(pat, val_reg, Some(next_block));

                    let res = self.lower_expr(expr);

                    self.builder.push_instr(ir::Inst::Assign(res_reg, res));
                    self.builder.finish_block(ir::Terminator::Jump(end_block));

                    self.builder.switch_to_block(next_block);
                }

                self.builder.switch_to_block(end_block);
                res_reg
            }
        }
    }

    fn lower_pat(
        &mut self,
        pat: ast::PatternId<'a>,
        reg: ir::Reg,
        fail_block: Option<ir::BlockId>,
    ) {
        match pat.data(self.db) {
            ast::PatternData::Wildcard => (),
            x @ (ast::PatternData::True | ast::PatternData::False) => {
                let t_reg = self.builder.new_reg();

                self.builder
                    .push_instr(ir::Inst::LoadBool(t_reg, x == ast::PatternData::True));

                let cond = self.builder.new_reg();
                self.builder.push_instr(ir::Inst::CmpEq(cond, reg, t_reg));

                let success = self.builder.new_block();

                self.builder.finish_block(ir::Terminator::BranchIf {
                    cond,
                    th: success,
                    el: fail_block.unwrap(),
                });

                self.builder.switch_to_block(success);
            }
            ast::PatternData::Tuple(pattern_ids) => todo!(),
            ast::PatternData::Var { name, is_mut: _ } => {
                self.extend(name, reg);
            }
        }
    }

    fn lookup(&self, ident: ast::Ident<'_>) -> ir::Reg {
        for scope in self.scopes.iter().rev() {
            if let Some(tp) = scope.get(&ident) {
                return tp.clone();
            }
        }
        panic!()
    }

    pub fn extend(&mut self, x: ast::Ident<'a>, reg: ir::Reg) {
        self.scopes.last_mut().unwrap().insert(x, reg);
    }
}
