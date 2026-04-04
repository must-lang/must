use std::sync::Arc;

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum Type {
    Error,

    Int,
    Tuple(Vec<Type>),
    Bool,
    Array(usize, Arc<Type>),

    Fn(Vec<Type>, Arc<Type>),
    Ptr { tp: Arc<Type>, is_mut: bool },
    Slice { tp: Arc<Type>, is_mut: bool },
}

impl Type {
    pub fn fun(args: Vec<Type>, ret: Type) -> Self {
        Type::Fn(args, Arc::new(ret))
    }

    pub fn error() -> Self {
        Type::Error
    }

    pub(crate) fn int() -> Self {
        Type::Int
    }

    pub(crate) fn bool() -> Self {
        Type::Bool
    }

    pub(crate) fn tuple(tps: Vec<Type>) -> Self {
        Type::Tuple(tps)
    }

    pub(crate) fn array(size: usize, tp: Type) -> Self {
        Type::Array(size, Arc::new(tp))
    }

    pub(crate) fn ptr(tp: Type, is_mut: bool) -> Self {
        Type::Ptr {
            tp: Arc::new(tp),
            is_mut,
        }
    }

    pub(crate) fn slice(tp: Type, is_mut: bool) -> Self {
        Type::Slice {
            tp: Arc::new(tp),
            is_mut,
        }
    }
}
