use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    localex_daemon::main().await
}
