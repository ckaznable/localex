use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let param = tocalex::cli::Cli::parse();
    tocalex::main(param.into()).await
}
