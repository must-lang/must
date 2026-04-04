use salsa::Database;

use crate::{
    def_map::FunctionId,
    parser::{ast, func_ast},
    tp::Type,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct FnSignature {
    pub args: Vec<Type>,
    pub ret: Type,
}

impl FnSignature {
    pub fn as_type(self) -> Type {
        Type::fun(self.args, self.ret)
    }
}

#[salsa::tracked]
pub fn func_signature<'db>(db: &'db dyn Database, f: FunctionId<'db>) -> FnSignature {
    let fn_def = func_ast(db, f);
    let args = fn_def
        .args(db)
        .iter()
        .map(|(_, tp)| parse_type(db, *tp))
        .collect();
    let ret = parse_type(db, fn_def.ret_type(db));
    FnSignature { args, ret }
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
        ast::TypeExprData::Slice(is_mut, tp) => Type::slice(parse_type(db, tp), is_mut),
    }
}
