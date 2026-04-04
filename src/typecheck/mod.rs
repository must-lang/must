use std::{collections::HashMap, sync::Arc};

use salsa::{Accumulator, Database};

mod error;
mod inference;

use crate::{
    def_map::{self, Function},
    diagnostic::Diagnostic,
    mod_tree::mod_tree,
    parser::ast::{self, ExprId},
    resolve::func_signature,
    tp::Type,
    typecheck::inference::{InferenceCtx, UType, UTypeView, UnifVar},
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct InferenceResult<'db> {
    pub inferred_types: HashMap<ExprId<'db>, Type>,
    pub coercions: HashMap<ExprId<'db>, Coercion>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum Coercion {
    /// Cast `*mut [N]T` to `[]mut T` (Thin pointer to Fat pointer)
    ArrayPtrToSlice,
}

#[salsa::tracked]
pub fn check_crate<'db>(db: &'db dyn salsa::Database, c: crate::parser::Crate) -> Option<()> {
    let tree = mod_tree(db, c);
    for module_id in tree.keys() {
        // 3. Get the DefMap for this module
        let def_map = def_map::module_def_map(db, *module_id)?;

        // 4. Look at everything defined in this module
        for f in def_map.functions.values() {
            check_fn(db, *f);
        }
    }
    Some(())
}

#[salsa::tracked]
pub fn check_fn<'db>(db: &'db dyn Database, f: Function<'db>) -> InferenceResult<'db> {
    let mut ctx = InferenceCtx::new(db, f);
    let sig = func_signature(db, f);

    let fn_ast = f.ast(db);

    for ((pat, _), tp) in fn_ast.args(db).into_iter().zip(sig.args) {
        for (name, tp, is_mut) in ctx.check_pat(pat, &tp.into()) {
            ctx.extend(name, tp, is_mut);
        }
    }

    if let Some(body) = fn_ast.body(db) {
        ctx.check_expr(body, &sig.ret.into(), false);
    }

    ctx.finish()
}

impl<'db> InferenceCtx<'db> {
    fn finish(mut self) -> InferenceResult<'db> {
        let types = self
            .inferred_types
            .clone()
            .into_iter()
            .map(|(id, tp)| (id, self.seal_type(tp)))
            .collect();
        InferenceResult {
            inferred_types: types,
            coercions: self.coercions,
        }
    }

    fn seal_type(&mut self, tp: UType) -> Type {
        match self.view(&tp) {
            UTypeView::Error => Type::Error,
            UTypeView::Tuple(tps) => {
                Type::Tuple(tps.into_iter().map(|tp| self.seal_type(tp)).collect())
            }
            UTypeView::Int => Type::Int,
            UTypeView::Bool => Type::Bool,
            UTypeView::UnifVar(_) => Type::Error,
            UTypeView::Fn(args, ret) => {
                let args = args.into_iter().map(|tp| self.seal_type(tp)).collect();
                let ret = self.seal_type(ret);
                Type::Fn(args, Arc::new(ret))
            }
            UTypeView::Ptr { tp, is_mut } => Type::Ptr {
                tp: Arc::new(self.seal_type(tp)),
                is_mut,
            },
            UTypeView::Array(size, tp) => Type::Array(size, Arc::new(self.seal_type(tp))),
            UTypeView::Slice { tp, is_mut } => Type::Slice {
                tp: Arc::new(self.seal_type(tp)),
                is_mut,
            },
        }
    }

    fn check_expr(&mut self, e: ast::ExprId<'db>, exp: &UType, exp_mut: bool) {
        match e.data(self.db) {
            ast::ExprData::Let(pat, e1, e2) => {
                let (tp, _) = self.with_scope(|ctx| ctx.infer_expr(e1));
                let bindings = self.check_pat(pat, &tp);
                for (x, tp, is_mut) in bindings {
                    self.extend(x, tp, is_mut);
                }
                self.check_expr(e2, exp, exp_mut);
                self.inferred_types.insert(e, exp.clone());
            }
            _ => {
                let (got, m) = self.infer_expr(e);
                if !self.coerce(Some(e), &got, exp) {
                    Diagnostic::type_mismatch(
                        self.db,
                        e.span(self.db),
                        self.seal_type(exp.clone()),
                        self.seal_type(got),
                    )
                    .accumulate(self.db);
                }
                if exp_mut && !m {
                    Diagnostic::exp_mut(self.db, e.span(self.db)).accumulate(self.db);
                }
            }
        }
    }

    fn infer_expr(&mut self, e: ast::ExprId<'db>) -> (UType, bool) {
        let tp = match e.data(self.db) {
            ast::ExprData::Error => (UType::error(), true),
            ast::ExprData::Num(_) => (UType::int(), false),
            ast::ExprData::Var(ident) => match self.lookup(ident) {
                Some(tp) => tp.clone(),
                None => {
                    Diagnostic::unbound_var(self.db, e.span(self.db), ident.text(self.db))
                        .accumulate(self.db);
                    (UType::error(), true)
                }
            },
            ast::ExprData::FnCall(ident, exprs) => match self.lookup(ident) {
                Some((tp, _)) => match self.view(&tp) {
                    UTypeView::Fn(args, ret) => {
                        let mut exprs_iter = exprs.into_iter();
                        let mut id = 0;
                        for arg in args {
                            id += 1;
                            if let Some(expr) = exprs_iter.next() {
                                self.check_expr(expr, &arg, false)
                            } else {
                                Diagnostic::missing_argument(
                                    self.db,
                                    id,
                                    e.span(self.db),
                                    self.seal_type(arg),
                                )
                                .accumulate(self.db);
                            }
                        }
                        for expr in exprs_iter {
                            id += 1;
                            Diagnostic::unexpected_argument(self.db, id, expr.span(self.db))
                                .accumulate(self.db);
                        }
                        (ret, false)
                    }
                    _ => todo!(),
                },
                None => {
                    Diagnostic::unbound_var(self.db, e.span(self.db), ident.text(self.db))
                        .accumulate(self.db);
                    (UType::error(), false)
                }
            },
            ast::ExprData::Let(pat, e1, e2) => {
                let (tp, _) = self.with_scope(|ctx| ctx.infer_expr(e1));
                let bindings = self.check_pat(pat, &tp);
                for (x, tp, is_mut) in bindings {
                    self.extend(x, tp, is_mut);
                }
                self.infer_expr(e2)
            }
            ast::ExprData::True | ast::ExprData::False => (UType::bool(), false),
            ast::ExprData::Match(expr, items) => {
                let (pat_tp, _) = self.infer_expr(expr);
                let tp = self.new_uvar();
                for (pat, expr) in items {
                    let bindings = self.check_pat(pat, &pat_tp);
                    for (x, tp, is_mut) in bindings {
                        self.extend(x, tp, is_mut);
                    }
                    self.check_expr(expr, &tp, false);
                }
                (tp, false)
            }
            ast::ExprData::Tuple(exprs) => {
                let tps = exprs.into_iter().map(|e| self.infer_expr(e).0).collect();
                (UType::tuple(tps), false)
            }
            ast::ExprData::Assign(e1, e2) => {
                let (tp, is_mut) = self.infer_expr(e1);
                if !is_mut {
                    Diagnostic::exp_mut(self.db, e1.span(self.db)).accumulate(self.db);
                }
                self.check_expr(e2, &tp, false);
                (UType::unit(), false)
            }
            ast::ExprData::AddressOf(e) => {
                let (tp, is_mut) = self.infer_expr(e);
                (UType::ptr(tp, is_mut), false)
            }
            ast::ExprData::Deref(e) => {
                let (tp, _) = self.infer_expr(e);
                match self.view(&tp) {
                    UTypeView::Ptr { tp, is_mut } => (tp, is_mut),
                    _ => {
                        Diagnostic::cannot_deref(self.db, e.span(self.db)).accumulate(self.db);
                        (UType::error(), true)
                    }
                }
            }
            ast::ExprData::Array(exprs) => {
                let size = exprs.len();
                let tp = self.new_uvar();
                for e in exprs {
                    self.check_expr(e, &tp, false);
                }
                (UType::array(size, tp), false)
            }
            ast::ExprData::Index(arr, id) => {
                self.check_expr(id, &UType::int(), false);
                let (arr_tp, is_mut) = self.infer_expr(arr);
                match self.view(&arr_tp) {
                    UTypeView::Array(_, tp) => (tp, is_mut),
                    UTypeView::Slice { tp, is_mut } => (tp, is_mut),
                    UTypeView::Error => (UType::error(), is_mut),
                    _ => {
                        Diagnostic::cannot_index(self.db, arr.span(self.db)).accumulate(self.db);
                        (UType::error(), is_mut)
                    }
                }
            }
            ast::ExprData::Seq(e1, e2) => {
                self.infer_expr(e1);
                self.infer_expr(e2)
            }
            ast::ExprData::BinOp(op, e1, e2) => {
                self.check_expr(e1, &UType::int(), false);
                self.check_expr(e2, &UType::int(), false);
                let tp = match op {
                    ast::Op::Add | ast::Op::Sub | ast::Op::Mul | ast::Op::Div => UType::int(),
                    ast::Op::Le | ast::Op::Eq => UType::bool(),
                };
                (tp, false)
            }
        };
        self.inferred_types.insert(e, tp.0.clone());
        tp
    }

    fn check_pat(
        &mut self,
        pat: ast::PatternId<'db>,
        tp: &UType,
    ) -> Vec<(ast::Ident<'db>, UType, bool)> {
        match pat.data(self.db) {
            ast::PatternData::Wildcard => vec![],
            ast::PatternData::True | ast::PatternData::False => {
                if !self.coerce(None, tp, &UType::bool()) {
                    Diagnostic::type_mismatch(
                        self.db,
                        pat.span(self.db),
                        self.seal_type(tp.clone()),
                        self.seal_type(UType::bool()),
                    )
                    .accumulate(self.db);
                }
                vec![]
            }
            ast::PatternData::Var { name, is_mut } => {
                vec![(name, tp.clone(), is_mut)]
            }
            ast::PatternData::Tuple(pats) => match self.view(tp) {
                UTypeView::Tuple(tps) => {
                    if pats.len() != tps.len() {
                        Diagnostic::type_mismatch(
                            self.db,
                            pat.span(self.db),
                            self.seal_type(tp.clone()),
                            self.seal_type(UType::tuple(vec![])),
                        )
                        .accumulate(self.db);
                        return vec![];
                    }

                    pats.into_iter()
                        .zip(tps)
                        .flat_map(|(pat, tp)| self.check_pat(pat, &tp))
                        .collect()
                }
                _ => {
                    Diagnostic::type_mismatch(
                        self.db,
                        pat.span(self.db),
                        self.seal_type(tp.clone()),
                        self.seal_type(UType::tuple(vec![])),
                    )
                    .accumulate(self.db);
                    vec![]
                }
            },
        }
    }

    fn coerce(&mut self, expr_id: Option<ast::ExprId<'db>>, from: &UType, to: &UType) -> bool {
        use UTypeView::*;
        let v1 = self.view(from);
        let v2 = self.view(to);
        match (v1, v2) {
            (Error, _) => true,
            (_, Error) => true,
            (Int, Int) => true,
            (Bool, Bool) => true,

            (
                Ptr {
                    tp: tp1,
                    is_mut: m1,
                },
                Ptr {
                    tp: tp2,
                    is_mut: m2,
                },
            ) => self.coerce(expr_id, &tp1, &tp2) && (m1 || !m2),

            (
                Slice {
                    tp: tp1,
                    is_mut: m1,
                },
                Slice {
                    tp: tp2,
                    is_mut: m2,
                },
            ) => self.coerce(expr_id, &tp1, &tp2) && (m1 || !m2),

            (
                Ptr {
                    tp: tp1,
                    is_mut: m1,
                },
                Slice {
                    tp: tp2,
                    is_mut: m2,
                },
            ) => match self.view(&tp1) {
                Array(_, tp1) => {
                    let b = self.coerce(None, &tp1, &tp2) && (m1 || !m2);

                    if let Some(id) = expr_id
                        && b
                    {
                        self.coercions.insert(id, Coercion::ArrayPtrToSlice);
                    }

                    b
                }
                _ => false,
            },

            // TODO: are arrays covariant? are there scary subtyping relations that can screw this up?
            (Array(s1, tp1), Array(s2, tp2)) => self.coerce(expr_id, &tp1, &tp2) && (s1 == s2),

            (Tuple(tps1), Tuple(tps2)) => {
                tps1.iter()
                    .zip(&tps2)
                    .all(|(tp1, tp2)| self.coerce(expr_id, tp1, tp2))
                    && tps1.len() == tps2.len()
            }

            // TODO: same issue as with arrays. Tuples too? and generally variance? i dont know
            (Fn(args1, tp1), Fn(args2, tp2)) => {
                args1
                    .iter()
                    .zip(&args2)
                    .all(|(tp1, tp2)| self.coerce(expr_id, tp1, tp2))
                    && args1.len() == args2.len()
                    && self.coerce(expr_id, &tp1, &tp2)
            }

            (UnifVar(u1), UnifVar(u2)) => {
                self.union(u1, u2);
                true
            }

            (UnifVar(u), tp) | (tp, UnifVar(u)) => {
                if !self.occurs(&tp.clone().wrap(), u) {
                    self.resolve(u, tp.wrap());
                    true
                } else {
                    false
                }
            }

            (_, _) => false,
        }
    }

    pub fn occurs(&mut self, tp: &UType, uv: UnifVar) -> bool {
        match self.view(tp) {
            UTypeView::Error | UTypeView::Bool | UTypeView::Int => false,
            UTypeView::UnifVar(unif_var) => uv == unif_var,
            UTypeView::Fn(args, ret) => {
                args.iter().any(|tp| self.occurs(tp, uv)) || self.occurs(&ret, uv)
            }
            UTypeView::Tuple(items) => items.iter().any(|tp| self.occurs(tp, uv)),
            UTypeView::Array(_, tp) | UTypeView::Ptr { tp, .. } | UTypeView::Slice { tp, .. } => {
                self.occurs(&tp, uv)
            }
        }
    }
}
