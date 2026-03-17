use std::{collections::HashMap, sync::Mutex};

use crate::parser::Source;

pub struct State {
    db: Mutex<salsa::DatabaseImpl>,
    file_map: Mutex<HashMap<String, Source>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(salsa::DatabaseImpl::new()),
            file_map: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_db(&self) -> salsa::DatabaseImpl {
        self.db.lock().unwrap().clone()
    }

    pub fn get_file(&self, name: &str) -> Option<Source> {
        self.file_map.lock().unwrap().get(name).cloned()
    }

    pub fn add_file(&self, name: String, source: Source) {
        self.file_map.lock().unwrap().insert(name, source);
    }
}
