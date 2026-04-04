use std::collections::HashMap;

use ena::unify::InPlaceUnificationTable;

use crate::{
    def_map::{FunctionId, module_def_map},
    parser::ast,
    resolve::func_signature,
    typecheck::Coercion,
};

pub struct InferenceCtx<'db> {
    unif: InPlaceUnificationTable<UnifVar>,
    scopes: Vec<HashMap<ast::Ident<'db>, (UType, bool)>>,
    f: FunctionId<'db>,
    pub db: &'db dyn salsa::Database,
    pub inferred_types: HashMap<ast::ExprId<'db>, UType>,
    pub coercions: HashMap<ast::ExprId<'db>, Coercion>,
}
impl<'db> InferenceCtx<'db> {
    pub fn new(db: &'db dyn salsa::Database, f: FunctionId<'db>) -> Self {
        Self {
            unif: InPlaceUnificationTable::new(),
            db,
            f,
            scopes: vec![HashMap::new()],
            inferred_types: HashMap::new(),
            coercions: HashMap::new(),
        }
    }
    pub(crate) fn union(&mut self, u1: UnifVar, u2: UnifVar) {
        self.unif.union(u1, u2);
    }

    pub(crate) fn resolve(&mut self, u: UnifVar, tp: UType) {
        self.unif.union_value(u, Some(tp));
    }

    pub fn lvl(&self) -> u32 {
        self.scopes.len() as u32
    }

    pub fn extend(&mut self, x: ast::Ident<'db>, tp: UType, is_mut: bool) {
        self.scopes.last_mut().unwrap().insert(x, (tp, is_mut));
    }

    pub fn lookup(&self, x: ast::Ident<'db>) -> Option<(UType, bool)> {
        for scope in self.scopes.iter().rev() {
            if let Some(tp) = scope.get(&x) {
                return Some(tp.clone());
            }
        }
        let m = self.f.module(self.db);
        let def_map = module_def_map(self.db, m)?;
        let f_id = def_map.functions.get(x.text(self.db))?;
        let sig = func_signature(self.db, *f_id);
        Some((sig.as_type().into(), false))
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
