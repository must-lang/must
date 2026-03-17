use ariadne::Source;
use salsa::DatabaseImpl;

use crate::{diagnostic::Diagnostic, parser, queries};

pub fn run(filename: String) {
    let text = std::fs::read_to_string(&filename).unwrap();
    let db = &DatabaseImpl::new();
    let source = parser::Source::new(db, text.clone());
    let _ = queries::compile_all(db, source);
    let diags: Vec<&Diagnostic> = queries::compile_all::accumulated::<Diagnostic>(db, source);
    for diag in diags {
        diag.as_ariadne_report(&filename)
            .eprint((&filename, Source::from(&text)))
            .unwrap()
    }
}
