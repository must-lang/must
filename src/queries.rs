use salsa::Database;

use crate::{parser, typecheck};

#[salsa::tracked]
pub fn compile_all<'db>(
    db: &'db dyn Database,
    source: parser::Source,
) -> Option<(typecheck::InferenceResult<'db>, parser::ast::File<'db>)> {
    let prog = parser::into_hir(db, source)?;
    let inference = typecheck::check_file(db, prog);
    Some((inference, prog))
}
