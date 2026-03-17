use std::{fmt::Display, ops::Range};

use lalrpop_util::ParseError;
use line_index::LineIndex;
use tower_lsp::lsp_types;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
}

#[salsa::accumulator]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub start_byte: usize,
    pub end_byte: usize,
    pub message: String,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn as_ariadne_report<'a>(
        &self,
        filename: &'a String,
    ) -> ariadne::Report<'a, (&'a String, Range<usize>)> {
        let mut builder = ariadne::Report::build(
            ariadne::ReportKind::Error,
            (filename, self.start_byte..self.end_byte),
        )
        .with_message(&self.message)
        .with_label(
            ariadne::Label::new((filename, self.start_byte..self.end_byte))
                .with_message(&self.message),
        );
        builder.with_notes(&self.notes);
        builder.finish()
    }

    pub fn as_lsp_diagnostic(&self, idx: &LineIndex) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: {
                    let line_col = idx.line_col((self.start_byte as u32).into());
                    lsp_types::Position {
                        line: line_col.line,
                        character: line_col.col,
                    }
                },
                end: {
                    let line_col = idx.line_col((self.end_byte as u32).into());
                    lsp_types::Position {
                        line: line_col.line,
                        character: line_col.col,
                    }
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: self.message.clone(),
            ..Default::default()
        }
    }

    pub fn parser_error<T: Display, E>(err: ParseError<usize, T, E>) -> Self {
        match err {
            lalrpop_util::ParseError::InvalidToken { location } => Self {
                severity: Severity::Error,
                start_byte: location,
                end_byte: location,
                message: "invalid token".into(),
                notes: vec![],
            },
            lalrpop_util::ParseError::UnrecognizedEof { location, expected } => Self {
                severity: Severity::Error,
                start_byte: location,
                end_byte: location,
                message: "unexpected end of file".into(),
                notes: vec![format!("expected one of:\n{}", expected.join("\n"))],
            },
            lalrpop_util::ParseError::UnrecognizedToken { token, expected } => Self {
                severity: Severity::Error,
                start_byte: token.0,
                end_byte: token.2,
                message: format!("unexpected token: {}", token.1),
                notes: vec![format!("expected one of:\n{}", expected.join("\n"))],
            },
            lalrpop_util::ParseError::ExtraToken { token } => Self {
                severity: Severity::Error,
                start_byte: token.0,
                end_byte: token.2,
                message: format!("unexpected token: {}", token.1),
                notes: vec![],
            },
            lalrpop_util::ParseError::User { .. } => {
                todo!("no user-defined error in the parser")
            }
        }
    }
}
