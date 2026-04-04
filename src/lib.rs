mod bytecode;
mod def_map;
mod diagnostic;
mod layout;
mod lsp;
mod mod_tree;
mod parser;
mod pipeline;
mod queries;
mod resolve;
mod span;
mod state;
mod tp;
mod typecheck;
mod vm;

pub use lsp::run as run_lsp;
pub use pipeline::compile_prog;
pub use pipeline::run as run_pipeline;
