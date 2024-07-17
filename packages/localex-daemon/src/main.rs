use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let param = localex_daemon::cli::Cli::parse();
    localex_daemon::main(param.into()).await
}
