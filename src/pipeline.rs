use ariadne::Source;
use salsa::DatabaseImpl;

use crate::{bytecode, diagnostic::Diagnostic, parser, queries, typecheck, vm};

pub fn compile_prog(filename: String) -> Result<bytecode::ir::Prog, usize> {
    let text = std::fs::read_to_string(&filename).unwrap();
    let db = &DatabaseImpl::new();
    let source = parser::Source::new(db, text.clone());

    let c = parser::Crate::new(db, source);
    let _ = typecheck::check_crate(db, c);
    let diags: Vec<&Diagnostic> = typecheck::check_crate::accumulated::<Diagnostic>(db, c);

    let err_count = diags.len();
    for diag in diags {
        diag.as_ariadne_report(&filename)
            .eprint((&filename, Source::from(&text)))
            .unwrap()
    }

    let result = bytecode::compile(db, c);

    if err_count == 0 {
        Ok(result.unwrap())
    } else {
        Err(err_count)
    }
}

pub fn run(filename: String) {
    match compile_prog(filename) {
        Ok(prog) => {
            let v = vm::run(prog);
            println!("Program evaluated to: {:#?}", v);
        }
        Err(n) => {
            eprintln!("{n} error occured. Compilation aborted")
        }
    }
}
