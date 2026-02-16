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
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

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

    // Apply timeouts matching the API's configuration to prevent hanging
    // during cluster instability or consensus formation
    let timeout_secs: u64 = std::env::var("QDRANT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let connect_timeout_secs: u64 = std::env::var("QDRANT_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    builder = builder
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(connect_timeout_secs));

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
/// Retries transient failures with exponential backoff to handle Qdrant
/// cluster consensus delays (common with 3+ replica clusters).
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

    // Not in cache, check with Qdrant (with retry for transient failures)
    match client.collection_info(collection_name).await {
        Ok(_) => {
            // Collection exists, add to cache
            let mut known = KNOWN_COLLECTIONS.write().await;
            known.insert(cache_key);
            debug!(
                collection = collection_name,
                "Collection exists, added to cache"
            );
            return Ok(());
        }
        Err(e) => {
            // Only proceed to creation if the error indicates the collection doesn't exist.
            // If it's a transient/network error, we'll let the create retry loop handle it.
            let err_str = e.to_string().to_lowercase();
            let is_not_found = err_str.contains("not found") || err_str.contains("doesn't exist");
            if !is_not_found {
                warn!(
                    collection = collection_name,
                    error = %e,
                    "collection_info failed with non-404 error, will attempt creation anyway"
                );
            }
        }
    }

    // Collection doesn't exist (or we couldn't check), create it with retry
    info!(
        collection = collection_name,
        vector_size = vector_size,
        distance = "Cosine",
        "Creating collection"
    );

    let max_attempts: u32 = std::env::var("QDRANT_COLLECTION_CREATE_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let mut last_error = None;

    for attempt in 1..=max_attempts {
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
                    attempt = attempt,
                    "Collection created successfully"
                );
                let mut known = KNOWN_COLLECTIONS.write().await;
                known.insert(cache_key);
                return Ok(());
            }
            Err(e) => {
                let error_str = e.to_string();

                // Handle race condition: collection may have been created by another worker
                if error_str.contains("already exists") {
                    info!(
                        collection = collection_name,
                        "Collection already exists (created by another worker), continuing"
                    );
                    let mut known = KNOWN_COLLECTIONS.write().await;
                    known.insert(cache_key);
                    return Ok(());
                }

                // Check if this is a retryable error (GRPC errors, timeouts, consensus issues)
                let error_lower = error_str.to_lowercase();
                let is_retryable = error_lower.contains("timeout")
                    || error_lower.contains("unavailable")
                    || error_lower.contains("connection")
                    || error_lower.contains("broken pipe")
                    || error_lower.contains("consensus")
                    || error_lower.contains("transport")
                    || error_lower.contains("internal");

                if is_retryable && attempt < max_attempts {
                    // Exponential backoff: 1s, 2s, 4s, 8s
                    let delay = Duration::from_secs(1u64 << (attempt - 1).min(4));
                    warn!(
                        collection = collection_name,
                        attempt = attempt,
                        max_attempts = max_attempts,
                        delay_secs = delay.as_secs(),
                        error = %e,
                        "Retryable error creating collection, will retry"
                    );
                    tokio::time::sleep(delay).await;
                    last_error = Some(error_str);
                    continue;
                }

                // Non-retryable or exhausted retries
                return Err(anyhow::anyhow!(
                    "Failed to create collection '{}' after {} attempt(s): {}",
                    collection_name,
                    attempt,
                    error_str
                ));
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to create collection '{}' after {} attempts: {}",
        collection_name,
        max_attempts,
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
}
