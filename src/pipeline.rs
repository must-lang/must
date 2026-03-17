use ariadne::Source;
use salsa::DatabaseImpl;

use crate::{diagnostic::Diagnostic, parser};

pub fn run(filename: String) {
    let text = std::fs::read_to_string(&filename).unwrap();
    let db = &DatabaseImpl::new();
    let source = parser::Source::new(db, text.clone());
    let _ = parser::into_hir(db, source);
    let diags: Vec<&Diagnostic> = parser::into_hir::accumulated::<Diagnostic>(db, source);
    for diag in diags {
        diag.as_ariadne_report(&filename)
            .eprint((&filename, Source::from(&text)))
            .unwrap()
    }
}
