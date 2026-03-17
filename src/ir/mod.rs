mod ast;

use std::sync::Arc;

pub use ast::*;
use salsa::Database;

use crate::parser;

#[salsa::tracked]
pub fn compile<'db>(db: &'db dyn Database, prog: parser::ast::File<'db>) -> ast::Program {
    let mut func = vec![];
    for def in prog.defs(db) {
        match def {
            parser::ast::Def::FnDef(fn_def) => {
                func.push(compile_func(db, *fn_def));
            }
        }
    }
    ast::Program { func }
}

#[salsa::tracked]
pub fn compile_func<'db>(db: &'db dyn Database, f: parser::ast::FnDef<'db>) -> ast::Function {
    ast::Function {
        name: f.name(db).text(db).clone(),
        args: f
            .args(db)
            .iter()
            .map(|(pat, _)| compile_pattern(db, *pat))
            .collect(),
        body: compile_expr(db, f.body(db)),
    }
}

#[salsa::tracked]
pub fn compile_expr<'db>(db: &'db dyn Database, e: parser::ast::ExprId<'db>) -> ast::Expr {
    match e.data(db) {
        parser::ast::ExprData::Error => panic!(),
        parser::ast::ExprData::Num(n) => ast::Expr::Num(n),
        parser::ast::ExprData::Var(ident) => ast::Expr::Var(ident.text(db).clone()),
        parser::ast::ExprData::FnCall(ident, expr_ids) => ast::Expr::FnCall(
            ident.text(db).clone(),
            Arc::new(expr_ids.into_iter().map(|e| compile_expr(db, e)).collect()),
        ),
        parser::ast::ExprData::Let(pat, e1, e2) => ast::Expr::Let(
            compile_pattern(db, pat),
            Arc::new(compile_expr(db, e1)),
            Arc::new(compile_expr(db, e2)),
        ),
        parser::ast::ExprData::Builtin(ident, expr_ids) => ast::Expr::Builtin(
            ident.text(db).clone(),
            Arc::new(expr_ids.into_iter().map(|e| compile_expr(db, e)).collect()),
        ),
        parser::ast::ExprData::True => ast::Expr::True,
        parser::ast::ExprData::False => ast::Expr::False,
        parser::ast::ExprData::Match(expr, items) => {
            let mut cls = vec![];
            for (pat, expr) in items {
                let pat = compile_pattern(db, pat);
                let expr = compile_expr(db, expr);
                cls.push((pat, expr))
            }
            ast::Expr::Match(Arc::new(compile_expr(db, expr)), cls)
        }
        parser::ast::ExprData::Tuple(exprs) => {
            ast::Expr::Tuple(exprs.into_iter().map(|e| compile_expr(db, e)).collect())
        }
        parser::ast::ExprData::Assign(expr1, expr2) => ast::Expr::Assign(
            Arc::new(compile_expr(db, expr1)),
            Arc::new(compile_expr(db, expr2)),
        ),
        parser::ast::ExprData::Store(expr1, expr2) => ast::Expr::Store(
            Arc::new(compile_expr(db, expr1)),
            Arc::new(compile_expr(db, expr2)),
        ),

        parser::ast::ExprData::AddressOf(e) => ast::Expr::AddressOf(Arc::new(compile_expr(db, e))),
        parser::ast::ExprData::Load(e) => ast::Expr::Load(Arc::new(compile_expr(db, e))),
        parser::ast::ExprData::Array(exprs) => {
            ast::Expr::Array(exprs.into_iter().map(|e| compile_expr(db, e)).collect())
        }
        parser::ast::ExprData::Index(expr1, expr2) => ast::Expr::Index(
            Arc::new(compile_expr(db, expr1)),
            Arc::new(compile_expr(db, expr2)),
        ),
    }
}

#[salsa::tracked]
pub fn compile_pattern<'db>(
    db: &'db dyn Database,
    pat: parser::ast::PatternId<'db>,
) -> ast::Pattern {
    match pat.data(db) {
        parser::ast::PatternData::Wildcard => ast::Pattern::Wildcard,
        parser::ast::PatternData::True => ast::Pattern::True,
        parser::ast::PatternData::False => ast::Pattern::False,
        parser::ast::PatternData::Var { name, .. } => ast::Pattern::Var(name.text(db).clone()),
        parser::ast::PatternData::Tuple(pats) => {
            ast::Pattern::Tuple(pats.into_iter().map(|e| compile_pattern(db, e)).collect())
        }
    }
}
