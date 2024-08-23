use database::LocalExDb;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    LocalExDb::new(None).await?;
    Ok(())
}
