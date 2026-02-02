//! Shared Qdrant client cache for workers.
//!
//! This module provides a thread-safe cache for Qdrant clients, keyed by URL.
//! Clients are reused across jobs to avoid connection overhead.

use once_cell::sync::Lazy;
use qdrant_client::Qdrant;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Global cache of Qdrant clients keyed by URL
static QDRANT_CLIENTS: Lazy<RwLock<HashMap<String, Arc<Qdrant>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Get or create a Qdrant client for the given URL and API key.
///
/// Clients are cached by URL. If a client for the given URL already exists,
/// it is returned. Otherwise, a new client is created and cached.
///
/// Note: API key changes for the same URL will use the cached client.
/// If you need to use different API keys for the same URL, consider using
/// different base URLs or not using the cache.
pub async fn get_or_create_client(
    url: &str,
    api_key: Option<String>,
) -> anyhow::Result<Arc<Qdrant>> {
    // Check if client exists in cache
    {
        let cache = QDRANT_CLIENTS.read().await;
        if let Some(client) = cache.get(url) {
            return Ok(Arc::clone(client));
        }
    }

    // Client not in cache, create a new one
    let mut builder = Qdrant::from_url(url);
    if let Some(key) = api_key {
        builder = builder.api_key(key);
    }

    let client = builder
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build Qdrant client: {e}"))?;
    let client = Arc::new(client);

    // Store in cache
    {
        let mut cache = QDRANT_CLIENTS.write().await;
        // Check again in case another task created it while we were building
        if let Some(existing) = cache.get(url) {
            return Ok(Arc::clone(existing));
        }
        cache.insert(url.to_string(), Arc::clone(&client));
        info!(url = url, "Created and cached new Qdrant client");
    }

    Ok(client)
}
