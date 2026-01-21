use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    ties::cli::run().await
}
