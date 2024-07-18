use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let param = localex_tui::cli::Cli::parse();
    localex_tui::main(param.into()).await
}
