use std::{hash::Hash, sync::Arc};

use ena::unify::{NoError, UnifyKey, UnifyValue};

use super::InferenceCtx;

#[derive(Copy, Clone, Debug, Eq)]
pub struct UnifVar {
    id: u32,
    lvl: u32,
}

impl PartialEq for UnifVar {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for UnifVar {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl UnifyValue for Type {
    type Error = NoError;

    fn unify_values(v1: &Self, v2: &Self) -> Result<Self, Self::Error> {
        unreachable!(
            "Context should never try to unify two concrete types: {:?}, {:?}",
            v1, v2
        )
    }
}

impl UnifyKey for UnifVar {
    type Value = Option<Type>;

    fn index(&self) -> u32 {
        self.id
    }

    fn from_index(id: u32) -> Self {
        UnifVar { id, lvl: 0 }
    }

    fn tag() -> &'static str {
        "uvar"
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub struct Type {
    data: Arc<TypeView>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum TypeView {
    Error,

    Int,
    Tuple(Vec<Type>),
    Bool,
    UnifVar(UnifVar),

    Array(usize, Type),

    Fn(Vec<Type>, Type),
    Ptr { tp: Type, is_mut: bool },
}

impl TypeView {
    pub fn wrap(self) -> Type {
        Type {
            data: Arc::new(self),
        }
    }
}

impl Type {
    pub fn unit() -> Self {
        TypeView::Tuple(vec![]).wrap()
    }
    pub fn fun(args: Vec<Type>, ret: Type) -> Self {
        TypeView::Fn(args, ret).wrap()
    }

    pub fn error() -> Self {
        TypeView::Error.wrap()
    }

    pub(crate) fn int() -> Self {
        TypeView::Int.wrap()
    }

    pub(crate) fn bool() -> Self {
        TypeView::Bool.wrap()
    }

    pub(crate) fn tuple(tps: Vec<Type>) -> Self {
        TypeView::Tuple(tps).wrap()
    }

    pub(crate) fn array(size: usize, tp: Type) -> Self {
        TypeView::Array(size, tp).wrap()
    }

    pub(crate) fn ptr(tp: Type, is_mut: bool) -> Type {
        TypeView::Ptr { tp, is_mut }.wrap()
    }
}

impl<'db> InferenceCtx<'db> {
    pub fn view(&mut self, tp: &Type) -> TypeView {
        match tp.data.as_ref() {
            TypeView::UnifVar(uvar) => {
                let root = self.unif.probe_value(*uvar);
                match root {
                    Some(tp) => self.view(&tp),
                    None => TypeView::UnifVar(*uvar),
                }
            }
            _ => (*tp.data).clone(),
        }
    }

    pub fn new_uvar(&mut self) -> Type {
        let mut k = self.unif.new_key(None);
        k.lvl = self.lvl();
        TypeView::UnifVar(k).wrap()
    }
}
