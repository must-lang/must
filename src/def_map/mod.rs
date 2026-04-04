use std::collections::HashMap;

use salsa::Database;

use crate::{
    mod_tree::{ModuleData, ModuleId, mod_tree},
    parser::{ast, parse_file},
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct DefMap<'db> {
    pub functions: HashMap<String, FunctionId<'db>>,
}

#[salsa::interned(debug)]
pub struct FunctionId {
    pub name: String,
    pub module: ModuleId<'db>,
}

#[salsa::tracked]
pub fn module_def_map<'db>(db: &'db dyn Database, m: ModuleId<'db>) -> Option<DefMap<'db>> {
    let c = m.c(db);
    let sf = c.root(db);

    let prog = parse_file(db, *sf)?;
    let mut functions = HashMap::new();
    for def in prog.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                let name = fn_def.name(db).text(db);
                let id = FunctionId::new(db, name, m);
                functions.insert(name.clone(), id);
            }
        }
    }
    Some(DefMap { functions })
}
