use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Clone)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(long)]
    pub log_file_name: Option<String>,

    #[arg(long)]
    pub sock: Option<PathBuf>,
}
