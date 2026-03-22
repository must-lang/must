mod diagnostic;
mod lsp;
mod parser;
mod pipeline;
mod queries;
mod span;
mod state;
mod typecheck;
mod vm;

pub use lsp::run as run_lsp;
pub use pipeline::compile_prog;
pub use pipeline::run as run_pipeline;
