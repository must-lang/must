use std::collections::HashMap;

use ena::unify::InPlaceUnificationTable;

use crate::{
    parser::ast,
    typecheck::{Coercion, DefMap},
};

pub struct InferenceCtx<'db> {
    unif: InPlaceUnificationTable<UnifVar>,
    scopes: Vec<HashMap<ast::Ident<'db>, (Type, bool)>>,
    def_map: DefMap<'db>,
    pub db: &'db dyn salsa::Database,
    pub type_map: HashMap<ast::ExprId<'db>, Type>,
    pub coercions: HashMap<ast::ExprId<'db>, Coercion>,
}
impl<'db> InferenceCtx<'db> {
    pub fn new(db: &'db dyn salsa::Database, idx: DefMap<'db>) -> Self {
        Self {
            unif: InPlaceUnificationTable::new(),
            db,
            scopes: vec![HashMap::new()],
            def_map: idx,
            type_map: HashMap::new(),
            coercions: HashMap::new(),
        }
    }
    pub(crate) fn union(&mut self, u1: UnifVar, u2: UnifVar) {
        self.unif.union(u1, u2);
    }

    pub(crate) fn resolve(&mut self, u: UnifVar, tp: Type) {
        self.unif.union_value(u, Some(tp));
    }

    pub fn lvl(&self) -> u32 {
        self.scopes.len() as u32
    }

    pub fn extend(&mut self, x: ast::Ident<'db>, tp: Type, is_mut: bool) {
        self.scopes.last_mut().unwrap().insert(x, (tp, is_mut));
    }

    pub fn lookup(&self, x: ast::Ident<'db>) -> Option<(Type, bool)> {
        for scope in self.scopes.iter().rev() {
            if let Some(tp) = scope.get(&x) {
                return Some(tp.clone());
            }
        }
        self.def_map
            .types(self.db)
            .get(x.text(self.db))
            .cloned()
            .map(|tp| (tp, false))
    }

    pub fn with_scope<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.scopes.push(HashMap::new());
        let r = f(self);
        self.scopes.pop();
        r
    }
}

mod tp;
pub use tp::*;
