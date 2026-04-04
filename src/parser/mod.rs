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
pub struct Crate {
    #[returns(ref)]
    pub root: Source,
}

#[salsa::tracked]
pub fn parse_file<'db>(db: &'db dyn Database, sf: Source) -> Option<ast::File<'db>> {
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
