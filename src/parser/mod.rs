use std::collections::BTreeMap;

use salsa::{Accumulator, Database};

use crate::diagnostic::Diagnostic;

pub mod ast;

lalrpop_util::lalrpop_mod!(pub parser, "/parser/parser.rs");

#[salsa::input(debug)]
pub struct Source {
    #[returns(ref)]
    pub text: String,
}

#[salsa::input(debug)]
pub struct Workspace {
    #[returns(ref)]
    pub files: BTreeMap<String, Source>,
}

#[salsa::tracked]
pub fn parse_workspace<'db>(db: &'db dyn Database, w: Workspace) -> ast::Workspace<'db> {
    let mut files = BTreeMap::new();
    for (name, source) in w.files(db).to_owned() {
        if let Some(file) = into_hir(db, source) {
            files.insert(name, file);
        }
    }
    ast::Workspace::new(db, files)
}

#[salsa::tracked]
pub fn into_hir<'db>(db: &'db dyn Database, sf: Source) -> Option<ast::File<'db>> {
    let parser = parser::FileParser::new();
    let input = sf.text(db);
    match parser.parse(db, input) {
        Ok(prog) => Some(prog),
        Err(err) => {
            Diagnostic::parser_error(err).accumulate(db);
            None
        }
    }
}
