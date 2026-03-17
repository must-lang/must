use clap::Parser;

use must::run_pipeline;

#[derive(clap::Parser)]
enum Cfg {
    /// Run a file.
    Run { file_name: String },
}

fn main() {
    let cfg = Cfg::parse();
    match cfg {
        Cfg::Run { file_name } => run_pipeline(file_name),
    }
}
