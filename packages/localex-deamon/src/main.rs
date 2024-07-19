use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let param = localex_deamon::cli::Cli::parse();
    localex_deamon::main(param.into()).await
}
