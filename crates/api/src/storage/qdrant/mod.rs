use anyhow::Result;
use qdrant_client::Qdrant;
use semantic_explorer_core::config::QdrantConfig;

pub(crate) async fn initialize_client(config: &QdrantConfig) -> Result<Qdrant> {
    let mut builder = Qdrant::from_url(&config.url)
        .timeout(config.timeout)
        .connect_timeout(config.connect_timeout);

    if let Some(ref api_key) = config.api_key {
        builder = builder.api_key(api_key.clone());
    }

    let client = builder.build()?;
    Ok(client)
}
