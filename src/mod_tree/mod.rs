use std::collections::HashMap;

use salsa::Database;

use crate::parser::{Crate, Source};

#[salsa::interned(debug)]
pub struct ModuleId {
    pub c: Crate,
    pub parent: Option<ModuleId<'db>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub enum ModuleOrigin {
    File(Source),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub struct ModuleData {
    pub origin: ModuleOrigin,
}

#[salsa::tracked]
pub fn mod_tree<'db>(db: &'db dyn Database, c: Crate) -> HashMap<ModuleId<'db>, ModuleData> {
    let id = ModuleId::new(db, c, None);
    let data = ModuleData {
        origin: ModuleOrigin::File(*c.root(db)),
    };
    let mut tree = HashMap::new();
    tree.insert(id, data);
    tree
}
