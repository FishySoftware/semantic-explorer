use anyhow::Result;
use qdrant_client::Qdrant;

pub(crate) async fn initialize_client() -> Result<Qdrant> {
    let url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let client = Qdrant::from_url(&url).build()?;
    Ok(client)
}
