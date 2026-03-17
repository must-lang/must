mod diagnostic;
mod lsp;
mod parser;
mod pipeline;
mod span;
mod state;

pub use lsp::run as run_lsp;
pub use pipeline::run as run_pipeline;
