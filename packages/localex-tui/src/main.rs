use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    localex_tui::main().await
}
