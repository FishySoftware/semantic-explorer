//! Shared Qdrant client cache for workers.
//!
//! This module provides:
//! - Thread-safe cache for Qdrant clients, keyed by URL
//! - Collection existence cache to avoid redundant collection_info() calls
//!
//! Clients and collection state are reused across jobs to avoid overhead.

use once_cell::sync::Lazy;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParams};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Global cache of Qdrant clients keyed by URL
static QDRANT_CLIENTS: Lazy<RwLock<HashMap<String, Arc<Qdrant>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Global cache of known collection names (verified to exist)
/// Key format: "{url}|{collection_name}"
static KNOWN_COLLECTIONS: Lazy<RwLock<HashSet<String>>> = Lazy::new(|| RwLock::new(HashSet::new()));

fn collection_cache_key(url: &str, collection_name: &str) -> String {
    format!("{}|{}", url, collection_name)
}

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

/// Ensure a collection exists, creating it if necessary.
/// Uses a local cache to avoid redundant collection_info() API calls.
///
/// Returns Ok if collection exists (or was created), Err on failure.
pub async fn ensure_collection_exists(
    client: &Arc<Qdrant>,
    url: &str,
    collection_name: &str,
    vector_size: u64,
) -> anyhow::Result<()> {
    let cache_key = collection_cache_key(url, collection_name);

    // Fast path: check cache first (read lock only)
    {
        let known = KNOWN_COLLECTIONS.read().await;
        if known.contains(&cache_key) {
            debug!(
                collection = collection_name,
                "Collection known to exist (cached)"
            );
            return Ok(());
        }
    }

    // Not in cache, check with Qdrant
    if client.collection_info(collection_name).await.is_ok() {
        // Collection exists, add to cache
        let mut known = KNOWN_COLLECTIONS.write().await;
        known.insert(cache_key);
        debug!(
            collection = collection_name,
            "Collection exists, added to cache"
        );
        return Ok(());
    }

    // Collection doesn't exist, create it
    info!(
        collection = collection_name,
        vector_size = vector_size,
        distance = "Cosine",
        "Creating collection"
    );

    let create_collection = CreateCollectionBuilder::new(collection_name)
        .vectors_config(VectorParams {
            size: vector_size,
            distance: Distance::Cosine.into(),
            on_disk: Some(true), // Store vectors on disk for large collections
            ..Default::default()
        })
        .on_disk_payload(true) // Store payloads on disk to reduce memory usage
        .build();

    match client.create_collection(create_collection).await {
        Ok(_) => {
            info!(
                collection = collection_name,
                "Collection created successfully"
            );
            // Add to cache
            let mut known = KNOWN_COLLECTIONS.write().await;
            known.insert(cache_key);
            Ok(())
        }
        Err(e) => {
            // Handle race condition: collection may have been created by another worker
            let error_str = e.to_string();
            if error_str.contains("already exists") {
                info!(
                    collection = collection_name,
                    "Collection already exists (created by another worker), continuing"
                );
                // Add to cache since it exists
                let mut known = KNOWN_COLLECTIONS.write().await;
                known.insert(cache_key);
                Ok(())
            } else {
                Err(anyhow::anyhow!("Failed to create collection: {}", e))
            }
        }
    }
}
