use salsa::Database;

use crate::{diagnostic::Diagnostic, span::Span, typecheck::SType};

impl Diagnostic {
    pub(super) fn unbound_var(db: &dyn Database, span: Span, name: &str) -> Self {
        Diagnostic::error(db, span, format!("unbound var: {:?}", name))
    }

    pub(super) fn exp_mut(db: &dyn Database, span: Span) -> Self {
        Diagnostic::error(db, span, "this expression cannot be mutated".to_string())
    }

    pub(super) fn cannot_index(db: &dyn Database, span: Span) -> Self {
        Diagnostic::error(db, span, "this expression cannot be indexed".to_string())
    }

    pub(super) fn cannot_deref(db: &dyn Database, span: Span) -> Self {
        Diagnostic::error(
            db,
            span,
            "this expression cannot be dereferenced".to_string(),
        )
    }

    pub(super) fn type_mismatch(db: &dyn Database, span: Span, exp: SType, got: SType) -> Self {
        Diagnostic::error(
            db,
            span,
            format!("type mismatch. expected: {:?}, got: {:?}", exp, got),
        )
    }

    pub(super) fn missing_argument(db: &dyn Database, id: usize, span: Span, tp: SType) -> Self {
        Diagnostic::error(db, span, format!("missing arg #{} of type {:?}", id, tp))
    }

    pub(super) fn unexpected_argument(db: &dyn Database, id: usize, span: Span) -> Self {
        Diagnostic::error(db, span, format!("unexpected arg #{}", id))
    }
}
