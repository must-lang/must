use salsa::{Accumulator, Database};

use crate::{def_map::FunctionId, diagnostic::Diagnostic, parser::ast::FnDef};

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

#[salsa::tracked]
pub fn func_ast<'db>(db: &'db dyn Database, f: FunctionId<'db>) -> FnDef<'db> {
    let m = f.module(db);
    let c = m.c(db);
    let sf = c.root(db);
    let prog = parse_file(db, *sf).unwrap();
    for def in prog.defs(db) {
        match def {
            ast::Def::FnDef(fn_def) => {
                let name = fn_def.name(db).text(db);
                if *name == f.name(db) {
                    return *fn_def;
                }
            }
        }
    }
    todo!()
}
