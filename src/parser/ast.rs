use std::collections::BTreeMap;

use crate::span::Span;

#[salsa::tracked(debug)]
pub struct Workspace<'db> {
    files: BTreeMap<String, File<'db>>,
}

#[salsa::tracked(debug)]
pub struct File<'db> {
    #[returns(ref)]
    pub defs: Vec<Def<'db>>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, salsa::Supertype, salsa::Update)]
pub enum Def<'db> {
    FnDef(FnDef<'db>),
}

#[salsa::tracked(debug)]
pub struct FnDef<'db> {
    pub ext: bool,
    pub name: Ident<'db>,
    #[tracked]
    pub args: Vec<(PatternId<'db>, TypeExprId<'db>)>,
    #[tracked]
    pub ret_type: TypeExprId<'db>,
    #[tracked]
    pub body: Option<ExprId<'db>>,
}

#[salsa::interned(debug)]
pub struct ExprId {
    pub data: ExprData<'db>,
    pub span: Span<'db>,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, salsa::Update)]
pub enum ExprData<'db> {
    Error,

    True,
    False,
    Num(usize),
    Var(Ident<'db>),
    FnCall(Ident<'db>, Vec<ExprId<'db>>),
    Let(PatternId<'db>, ExprId<'db>, ExprId<'db>),
    Seq(ExprId<'db>, ExprId<'db>),
    Assign(ExprId<'db>, ExprId<'db>),

    AddressOf(ExprId<'db>),
    Deref(ExprId<'db>),

    Tuple(Vec<ExprId<'db>>),

    Array(Vec<ExprId<'db>>),
    Index(ExprId<'db>, ExprId<'db>),

    Match(ExprId<'db>, Vec<(PatternId<'db>, ExprId<'db>)>),
}

#[salsa::interned(debug)]
pub struct PatternId {
    pub data: PatternData<'db>,
    pub span: Span<'db>,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, salsa::Update)]
pub enum PatternData<'db> {
    Wildcard,
    True,
    False,
    Tuple(Vec<PatternId<'db>>),
    Var { name: Ident<'db>, is_mut: bool },
}

#[salsa::interned(debug)]
pub struct TypeExprId {
    pub data: TypeExprData<'db>,
    pub span: Span<'db>,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, salsa::Update)]
pub enum TypeExprData<'db> {
    Error,

    Tuple(Vec<TypeExprId<'db>>),
    Array(usize, TypeExprId<'db>),
    Int,
    Bool,
    Ptr { tp: TypeExprId<'db>, is_mut: bool },
    Fn(Vec<TypeExprId<'db>>, TypeExprId<'db>),
}

#[salsa::interned(debug)]
pub struct Ident {
    #[returns(ref)]
    pub text: String,
}
