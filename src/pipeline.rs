use ariadne::Source;
use salsa::DatabaseImpl;

use crate::{diagnostic::Diagnostic, parser, queries, vm};

pub fn run(filename: String) {
    let text = std::fs::read_to_string(&filename).unwrap();
    let db = &DatabaseImpl::new();
    let source = parser::Source::new(db, text.clone());
    let result = queries::compile_all(db, source);
    let diags: Vec<&Diagnostic> = queries::compile_all::accumulated::<Diagnostic>(db, source);
    let err_count = diags.len();
    for diag in diags {
        diag.as_ariadne_report(&filename)
            .eprint((&filename, Source::from(&text)))
            .unwrap()
    }

    if let Some((_, prog)) = result
        && err_count == 0
    {
        let prog = vm::lower::compile(db, prog);
        let v = vm::run(prog);
        println!("Program evaluated to: {:#?}", v);
    } else {
        eprintln!("Errors occured. Compilation aborted.")
    }
}
