use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use salsa::{Accumulator, Database};

mod error;
mod inference;

use crate::{
    diagnostic::Diagnostic,
    parser::ast::{self, ExprId},
    typecheck::inference::{InferenceCtx, Type, TypeView, UnifVar},
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct InferenceResult<'db> {
    pub types: HashMap<ExprId<'db>, SType>,
}

#[salsa::tracked(debug)]
pub struct DefMap<'db> {
    types: BTreeMap<String, Type>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum SType {
    Error,

    Int,
    Tuple(Vec<SType>),
    Bool,
    UnifVar(UnifVar),

    Array(usize, Arc<SType>),

    Fn(Vec<SType>, Arc<SType>),
    Ptr { tp: Arc<SType>, is_mut: bool },
}

#[salsa::tracked]
pub fn check_file<'db>(db: &'db dyn Database, sf: ast::File<'db>) -> InferenceResult<'db> {
    let mut types = BTreeMap::new();
    for def in sf.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                let tp = parse_fn_type(db, *fn_def);
                let name = fn_def.name(db).text(db).clone();
                types.insert(name, tp);
            }
        }
    }
    let def_idx = DefMap::new(db, types);
    let mut types: HashMap<ExprId<'db>, _> = HashMap::new();
    for def in sf.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                types.extend(check_fn(db, *fn_def, def_idx).types);
            }
        }
    }
    InferenceResult { types }
}

#[salsa::tracked]
fn parse_fn_type<'db>(db: &'db dyn Database, fn_def: ast::FnDef<'db>) -> Type {
    let args = fn_def
        .args(db)
        .iter()
        .map(|(_, tp)| parse_type(db, *tp))
        .collect();
    let ret = parse_type(db, fn_def.ret_type(db));
    Type::fun(args, ret)
}

#[salsa::tracked]
fn parse_type<'db>(db: &'db dyn Database, tp: ast::TypeExprId<'db>) -> Type {
    match tp.data(db) {
        ast::TypeExprData::Error => Type::error(),
        ast::TypeExprData::Int => Type::int(),
        ast::TypeExprData::Bool => Type::bool(),
        ast::TypeExprData::Fn(args, ret) => Type::fun(
            args.into_iter().map(|tp| parse_type(db, tp)).collect(),
            parse_type(db, ret),
        ),
        ast::TypeExprData::Tuple(types) => {
            Type::tuple(types.into_iter().map(|tp| parse_type(db, tp)).collect())
        }
        ast::TypeExprData::Ptr { tp, is_mut } => Type::ptr(parse_type(db, tp), is_mut),
        ast::TypeExprData::Array(size, tp) => Type::array(size, parse_type(db, tp)),
    }
}

#[salsa::tracked]
pub fn check_fn<'db>(
    db: &'db dyn Database,
    f: ast::FnDef<'db>,
    def_idx: DefMap<'db>,
) -> InferenceResult<'db> {
    let mut ctx = InferenceCtx::new(db, def_idx);
    for (pat, tp) in f.args(db) {
        let tp = parse_type(db, tp);
        for (name, tp, is_mut) in ctx.check_pat(pat, &tp) {
            ctx.extend(name, tp, is_mut);
        }
    }
    let exp = parse_type(db, f.ret_type(db));
    if let Some(body) = f.body(db) {
        ctx.check_expr(body, &exp, false);
    }
    ctx.finish()
}

impl<'db> InferenceCtx<'db> {
    fn finish(mut self) -> InferenceResult<'db> {
        let types = self
            .type_map
            .clone()
            .into_iter()
            .map(|(id, tp)| (id, self.seal_type(tp)))
            .collect();
        InferenceResult { types }
    }

    fn seal_type(&mut self, tp: Type) -> SType {
        match self.view(&tp) {
            TypeView::Error => SType::Error,
            TypeView::Tuple(tps) => {
                SType::Tuple(tps.into_iter().map(|tp| self.seal_type(tp)).collect())
            }
            TypeView::Int => SType::Int,
            TypeView::Bool => SType::Bool,
            TypeView::UnifVar(unif_var) => SType::UnifVar(unif_var),
            TypeView::Fn(args, ret) => {
                let args = args.into_iter().map(|tp| self.seal_type(tp)).collect();
                let ret = self.seal_type(ret);
                SType::Fn(args, Arc::new(ret))
            }
            TypeView::Ptr { tp, is_mut } => SType::Ptr {
                tp: Arc::new(self.seal_type(tp)),
                is_mut,
            },
            TypeView::Array(size, tp) => SType::Array(size, Arc::new(self.seal_type(tp))),
        }
    }

    fn check_expr(&mut self, e: ast::ExprId<'db>, exp: &Type, exp_mut: bool) {
        match e.data(self.db) {
            ast::ExprData::Let(pat, e1, e2) => {
                let (tp, _) = self.with_scope(|ctx| ctx.infer_expr(e1));
                let bindings = self.check_pat(pat, &tp);
                for (x, tp, is_mut) in bindings {
                    self.extend(x, tp, is_mut);
                }
                self.check_expr(e2, exp, exp_mut);
                self.type_map.insert(e, exp.clone());
            }
            _ => {
                let (got, m) = self.infer_expr(e);
                if !self.coerce(&got, exp) {
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

    fn infer_expr(&mut self, e: ast::ExprId<'db>) -> (Type, bool) {
        let tp = match e.data(self.db) {
            ast::ExprData::Error => (Type::error(), true),
            ast::ExprData::Num(_) => (Type::int(), false),
            ast::ExprData::Var(ident) => match self.lookup(ident) {
                Some(tp) => tp.clone(),
                None => {
                    Diagnostic::unbound_var(self.db, e.span(self.db), ident.text(self.db))
                        .accumulate(self.db);
                    (Type::error(), true)
                }
            },
            ast::ExprData::FnCall(ident, exprs) => match self.lookup(ident) {
                Some((tp, _)) => match self.view(&tp) {
                    TypeView::Fn(args, ret) => {
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
                    (Type::error(), false)
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
            ast::ExprData::True | ast::ExprData::False => (Type::bool(), false),
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
                (Type::tuple(tps), false)
            }
            ast::ExprData::Assign(e1, e2) => {
                let (tp, is_mut) = self.infer_expr(e1);
                if !is_mut {
                    Diagnostic::exp_mut(self.db, e1.span(self.db)).accumulate(self.db);
                }
                self.check_expr(e2, &tp, false);
                (Type::unit(), false)
            }
            ast::ExprData::AddressOf(e) => {
                let (tp, is_mut) = self.infer_expr(e);
                (Type::ptr(tp, is_mut), false)
            }
            ast::ExprData::Deref(e) => {
                let (tp, _) = self.infer_expr(e);
                match self.view(&tp) {
                    TypeView::Ptr { tp, is_mut } => (tp, is_mut),
                    _ => {
                        Diagnostic::cannot_deref(self.db, e.span(self.db)).accumulate(self.db);
                        (Type::error(), true)
                    }
                }
            }
            ast::ExprData::Array(exprs) => {
                let size = exprs.len();
                let tp = self.new_uvar();
                for e in exprs {
                    self.check_expr(e, &tp, false);
                }
                (Type::array(size, tp), false)
            }
            ast::ExprData::Index(arr, id) => {
                self.check_expr(id, &Type::int(), false);
                let (arr_tp, is_mut) = self.infer_expr(arr);
                match self.view(&arr_tp) {
                    TypeView::Array(_, tp) => (tp, is_mut),
                    TypeView::Error => (Type::error(), is_mut),
                    _ => {
                        Diagnostic::cannot_index(self.db, arr.span(self.db)).accumulate(self.db);
                        (Type::error(), is_mut)
                    }
                }
            }
            ast::ExprData::Seq(e1, e2) => {
                self.infer_expr(e1);
                self.infer_expr(e2)
            }
            ast::ExprData::BinOp(op, e1, e2) => {
                self.check_expr(e1, &Type::int(), false);
                self.check_expr(e2, &Type::int(), false);
                let tp = match op {
                    ast::Op::Add | ast::Op::Sub | ast::Op::Mul | ast::Op::Div => Type::int(),
                    ast::Op::Le | ast::Op::Eq => Type::bool(),
                };
                (tp, false)
            }
        };
        self.type_map.insert(e, tp.0.clone());
        tp
    }

    fn check_pat(
        &mut self,
        pat: ast::PatternId<'db>,
        tp: &Type,
    ) -> Vec<(ast::Ident<'db>, Type, bool)> {
        match pat.data(self.db) {
            ast::PatternData::Wildcard => vec![],
            ast::PatternData::True | ast::PatternData::False => {
                if !self.coerce(tp, &Type::bool()) {
                    Diagnostic::type_mismatch(
                        self.db,
                        pat.span(self.db),
                        self.seal_type(tp.clone()),
                        self.seal_type(Type::bool()),
                    )
                    .accumulate(self.db);
                }
                vec![]
            }
            ast::PatternData::Var { name, is_mut } => {
                vec![(name, tp.clone(), is_mut)]
            }
            ast::PatternData::Tuple(pats) => match self.view(tp) {
                TypeView::Tuple(tps) => {
                    if pats.len() != tps.len() {
                        Diagnostic::type_mismatch(
                            self.db,
                            pat.span(self.db),
                            self.seal_type(tp.clone()),
                            self.seal_type(Type::tuple(vec![])),
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
                        self.seal_type(Type::tuple(vec![])),
                    )
                    .accumulate(self.db);
                    vec![]
                }
            },
        }
    }

    fn coerce(&mut self, from: &Type, to: &Type) -> bool {
        use TypeView::*;
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
            ) => self.coerce(&tp1, &tp2) && (m1 || !m2),

            // TODO: are arrays covariant? are there scary subtyping relations that can screw this up?
            (Array(s1, tp1), Array(s2, tp2)) => self.coerce(&tp1, &tp2) && (s1 == s2),

            (Tuple(tps1), Tuple(tps2)) => {
                tps1.iter()
                    .zip(&tps2)
                    .all(|(tp1, tp2)| self.coerce(tp1, tp2))
                    && tps1.len() == tps2.len()
            }

            // TODO: same issue as with arrays. Tuples too? and generally variance? i dont know
            (Fn(args1, tp1), Fn(args2, tp2)) => {
                args1
                    .iter()
                    .zip(&args2)
                    .all(|(tp1, tp2)| self.coerce(tp1, tp2))
                    && args1.len() == args2.len()
                    && self.coerce(&tp1, &tp2)
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

    pub fn occurs(&mut self, tp: &Type, uv: UnifVar) -> bool {
        match self.view(tp) {
            TypeView::Error | TypeView::Bool | TypeView::Int => false,
            TypeView::UnifVar(unif_var) => uv == unif_var,
            TypeView::Fn(args, ret) => {
                args.iter().any(|tp| self.occurs(tp, uv)) || self.occurs(&ret, uv)
            }
            TypeView::Tuple(items) => items.iter().any(|tp| self.occurs(tp, uv)),
            TypeView::Array(_, tp) => self.occurs(&tp, uv),
            TypeView::Ptr { tp, .. } => self.occurs(&tp, uv),
        }
    }
}
