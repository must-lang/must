use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub func: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,

    pub args: Vec<Pattern>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Tuple(Vec<Expr>),
    True,
    False,
    Num(usize),
    Var(String),
    FnCall(String, Arc<Vec<Expr>>),
    Builtin(String, Arc<Vec<Expr>>),
    Let(Pattern, Arc<Expr>, Arc<Expr>),
    Match(Arc<Expr>, Vec<(Pattern, Expr)>),
    Assign(Arc<Expr>, Arc<Expr>),
    AddressOf(Arc<Expr>),
    Load(Arc<Expr>),
    Store(Arc<Expr>, Arc<Expr>),
    Array(Vec<Expr>),
    Index(Arc<Expr>, Arc<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,
    True,
    False,
    Tuple(Vec<Pattern>),
    Var(String),
}
