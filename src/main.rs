use clap::Parser;

#[derive(clap::Parser)]
enum Cfg {
    /// Run in lsp mode.
    Lsp {
        /// Use stdio.
        #[arg(short, long, default_value_t = true)]
        stdio: bool,
    },
    /// Run a file.
    Run { file_name: String },
}

#[tokio::main]
async fn main() {
    let cfg = Cfg::parse();
    match cfg {
        Cfg::Lsp { .. } => must::run_lsp().await,
        Cfg::Run { file_name } => must::run_pipeline(file_name),
    }
}
