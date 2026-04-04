use std::{hash::Hash, sync::Arc};

use ena::unify::{NoError, UnifyKey, UnifyValue};

use crate::tp::Type;

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

impl UnifyValue for UType {
    type Error = NoError;

    fn unify_values(v1: &Self, v2: &Self) -> Result<Self, Self::Error> {
        unreachable!(
            "Context should never try to unify two concrete types: {:?}, {:?}",
            v1, v2
        )
    }
}

impl UnifyKey for UnifVar {
    type Value = Option<UType>;

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
pub struct UType {
    data: Arc<UTypeView>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, salsa::Update)]
pub enum UTypeView {
    Error,

    Int,
    Tuple(Vec<UType>),
    Bool,
    UnifVar(UnifVar),

    Array(usize, UType),

    Fn(Vec<UType>, UType),
    Ptr { tp: UType, is_mut: bool },
    Slice { tp: UType, is_mut: bool },
}

impl UTypeView {
    pub fn wrap(self) -> UType {
        UType {
            data: Arc::new(self),
        }
    }
}

impl UType {
    pub fn unit() -> Self {
        UTypeView::Tuple(vec![]).wrap()
    }
    pub fn fun(args: Vec<UType>, ret: UType) -> Self {
        UTypeView::Fn(args, ret).wrap()
    }

    pub fn error() -> Self {
        UTypeView::Error.wrap()
    }

    pub(crate) fn int() -> Self {
        UTypeView::Int.wrap()
    }

    pub(crate) fn bool() -> Self {
        UTypeView::Bool.wrap()
    }

    pub(crate) fn tuple(tps: Vec<UType>) -> Self {
        UTypeView::Tuple(tps).wrap()
    }

    pub(crate) fn array(size: usize, tp: UType) -> Self {
        UTypeView::Array(size, tp).wrap()
    }

    pub(crate) fn ptr(tp: UType, is_mut: bool) -> Self {
        UTypeView::Ptr { tp, is_mut }.wrap()
    }

    pub(crate) fn slice(tp: UType, is_mut: bool) -> Self {
        UTypeView::Slice { tp, is_mut }.wrap()
    }
}

impl<'db> InferenceCtx<'db> {
    pub fn view(&mut self, tp: &UType) -> UTypeView {
        match tp.data.as_ref() {
            UTypeView::UnifVar(uvar) => {
                let root = self.unif.probe_value(*uvar);
                match root {
                    Some(tp) => self.view(&tp),
                    None => UTypeView::UnifVar(*uvar),
                }
            }
            _ => (*tp.data).clone(),
        }
    }

    pub fn new_uvar(&mut self) -> UType {
        let mut k = self.unif.new_key(None);
        k.lvl = self.lvl();
        UTypeView::UnifVar(k).wrap()
    }
}

impl From<Type> for UType {
    fn from(value: Type) -> Self {
        match value {
            Type::Error => UType::error(),
            Type::Int => UType::int(),
            Type::Tuple(tps) => UType::tuple(tps.into_iter().map(Into::into).collect()),
            Type::Bool => UType::bool(),
            Type::Array(n, tp) => UType::array(n, ((*tp).clone()).into()),
            Type::Fn(args, ret) => UType::fun(
                args.into_iter().map(Into::into).collect(),
                ((*ret).clone()).into(),
            ),
            Type::Ptr { tp, is_mut } => UType::ptr(((*tp).clone()).into(), is_mut),
            Type::Slice { tp, is_mut } => UType::slice(((*tp).clone()).into(), is_mut),
        }
    }
}
