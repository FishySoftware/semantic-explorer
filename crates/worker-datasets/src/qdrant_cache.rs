//! Shared Qdrant client cache for workers.
//!
//! This module provides:
//! - Thread-safe, bounded cache for Qdrant clients, keyed by `(url, api_key)`
//! - Bounded collection-existence cache to avoid redundant `collection_exists` calls
//!
//! Clients and collection state are reused across jobs to avoid overhead. Both caches
//! are backed by `moka` and evict under an LRU policy once their capacity is exceeded.

use moka::sync::Cache;
use once_cell::sync::Lazy;
use qdrant_client::Qdrant;
use qdrant_client::QdrantError;
use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParams};
use semantic_explorer_core::config::QdrantCacheConfig;
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Cache sizing, set once from `main` via [`init_cache_config`].
static CACHE_CONFIG: OnceLock<QdrantCacheConfig> = OnceLock::new();

/// Initialize cache sizing from centralized config. Call once from main, before
/// any cache is first accessed.
pub fn init_cache_config(config: QdrantCacheConfig) {
    let _ = CACHE_CONFIG.set(config);
}

/// Resolve cache sizing, falling back to env/defaults if `init_cache_config` was
/// never called (e.g. in tests).
fn cache_config() -> &'static QdrantCacheConfig {
    CACHE_CONFIG.get_or_init(|| {
        QdrantCacheConfig::from_env().unwrap_or_else(|e| {
            warn!("Failed to load Qdrant cache config: {e} — using defaults");
            QdrantCacheConfig::default()
        })
    })
}

/// Global cache of Qdrant clients keyed by URL and API key hash.
static QDRANT_CLIENTS: Lazy<Cache<String, Arc<Qdrant>>> = Lazy::new(|| {
    Cache::builder()
        .max_capacity(cache_config().client_cache_capacity)
        .build()
});

/// Global cache of known collection names (verified to exist).
/// Key format: "{url}|{collection_name}".
static KNOWN_COLLECTIONS: Lazy<Cache<String, ()>> = Lazy::new(|| {
    Cache::builder()
        .max_capacity(cache_config().collection_cache_capacity)
        .build()
});

fn collection_cache_key(url: &str, collection_name: &str) -> String {
    format!("{url}|{collection_name}")
}

/// Cache key for a Qdrant client. The API key is hashed (never stored in plaintext)
/// so that the same URL accessed with different credentials yields distinct clients.
fn client_cache_key(url: &str, api_key: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.unwrap_or("").as_bytes());
    format!("{url}|{}", hex::encode(hasher.finalize()))
}

// Canonical gRPC status codes (https://grpc.io/docs/guides/status-codes/).
// Compared numerically so we don't depend on tonic directly — Qdrant's errors
// carry a different tonic version than the rest of the workspace.
const GRPC_CANCELLED: i32 = 1;
const GRPC_UNKNOWN: i32 = 2;
const GRPC_DEADLINE_EXCEEDED: i32 = 4;
const GRPC_ALREADY_EXISTS: i32 = 6;
const GRPC_RESOURCE_EXHAUSTED: i32 = 8;
const GRPC_ABORTED: i32 = 10;
const GRPC_INTERNAL: i32 = 13;
const GRPC_UNAVAILABLE: i32 = 14;

/// gRPC status code carried by a Qdrant error, if any.
fn grpc_code(err: &QdrantError) -> Option<i32> {
    match err {
        QdrantError::ResponseError { status }
        | QdrantError::ResourceExhaustedError { status, .. } => Some(status.code() as i32),
        _ => None,
    }
}

fn is_already_exists(err: &QdrantError) -> bool {
    grpc_code(err) == Some(GRPC_ALREADY_EXISTS)
}

/// Whether an error is transient and worth retrying (network blips, consensus
/// delays, timeouts, resource exhaustion).
pub(crate) fn is_retryable(err: &QdrantError) -> bool {
    if matches!(err, QdrantError::Io(_)) {
        return true;
    }
    matches!(
        grpc_code(err),
        Some(
            GRPC_UNAVAILABLE
                | GRPC_DEADLINE_EXCEEDED
                | GRPC_ABORTED
                | GRPC_INTERNAL
                | GRPC_RESOURCE_EXHAUSTED
                | GRPC_UNKNOWN
                | GRPC_CANCELLED
        )
    )
}

/// Get or create a Qdrant client for the given URL and API key.
///
/// Clients are cached by `(url, api_key)`, so distinct per-tenant credentials
/// pointing at the same URL each get their own client. Concurrent requests for the
/// same key coalesce onto a single build.
pub fn get_or_create_client(url: &str, api_key: Option<String>) -> anyhow::Result<Arc<Qdrant>> {
    let key = client_cache_key(url, api_key.as_deref());
    QDRANT_CLIENTS
        .try_get_with(key, || build_client(url, api_key))
        .map_err(|e| anyhow::anyhow!("Failed to get or create Qdrant client: {e}"))
}

fn build_client(url: &str, api_key: Option<String>) -> anyhow::Result<Arc<Qdrant>> {
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

    let client = builder
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(connect_timeout_secs))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build Qdrant client: {e}"))?;

    info!(url = url, "Created and cached new Qdrant client");
    Ok(Arc::new(client))
}

/// Ensure a collection exists, creating it if necessary.
///
/// Uses a local cache to avoid redundant existence checks. Both the existence check
/// and the create path retry transient failures with exponential backoff to handle
/// Qdrant cluster consensus delays (common with 3+ replica clusters).
///
/// Returns Ok if the collection exists (or was created), Err on failure.
pub async fn ensure_collection_exists(
    client: &Arc<Qdrant>,
    url: &str,
    collection_name: &str,
    vector_size: u64,
    distance: Distance,
) -> anyhow::Result<()> {
    let cache_key = collection_cache_key(url, collection_name);

    if KNOWN_COLLECTIONS.contains_key(&cache_key) {
        debug!(
            collection = collection_name,
            "Collection known to exist (cached)"
        );
        return Ok(());
    }

    let max_attempts: u32 = std::env::var("QDRANT_COLLECTION_CREATE_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    for attempt in 1..=max_attempts {
        match client.collection_exists(collection_name).await {
            Ok(true) => {
                KNOWN_COLLECTIONS.insert(cache_key, ());
                debug!(collection = collection_name, "Collection exists");
                return Ok(());
            }
            Ok(false) => {}
            Err(e) => {
                if let Some(delay) = retry_delay(&e, attempt, max_attempts) {
                    warn!(
                        collection = collection_name,
                        attempt,
                        max_attempts,
                        delay_secs = delay.as_secs(),
                        error = %e,
                        "Retryable error checking collection existence, will retry"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "Failed to check collection '{collection_name}': {e}"
                ));
            }
        }

        info!(
            collection = collection_name,
            vector_size,
            distance = ?distance,
            attempt,
            "Creating collection"
        );

        let create_collection = CreateCollectionBuilder::new(collection_name)
            .vectors_config(VectorParams {
                size: vector_size,
                distance: distance.into(),
                on_disk: Some(true), // Store vectors on disk for large collections
                ..Default::default()
            })
            .on_disk_payload(true) // Store payloads on disk to reduce memory usage
            .build();

        match client.create_collection(create_collection).await {
            Ok(_) => {
                KNOWN_COLLECTIONS.insert(cache_key, ());
                info!(
                    collection = collection_name,
                    attempt, "Collection created successfully"
                );
                return Ok(());
            }
            // Race: collection was created by another worker between our check and create.
            Err(e) if is_already_exists(&e) => {
                KNOWN_COLLECTIONS.insert(cache_key, ());
                info!(
                    collection = collection_name,
                    "Collection already exists (created by another worker), continuing"
                );
                return Ok(());
            }
            Err(e) => {
                if let Some(delay) = retry_delay(&e, attempt, max_attempts) {
                    warn!(
                        collection = collection_name,
                        attempt,
                        max_attempts,
                        delay_secs = delay.as_secs(),
                        error = %e,
                        "Retryable error creating collection, will retry"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "Failed to create collection '{collection_name}' after {attempt} attempt(s): {e}"
                ));
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to ensure collection '{collection_name}' exists after {max_attempts} attempts"
    ))
}

/// Exponential backoff delay (1s, 2s, 4s, 8s, capped) for a retryable error, or
/// `None` if the error is not retryable or retries are exhausted.
fn retry_delay(err: &QdrantError, attempt: u32, max_attempts: u32) -> Option<Duration> {
    if attempt >= max_attempts || !is_retryable(err) {
        return None;
    }
    Some(Duration::from_secs(1u64 << (attempt - 1).min(4)))
}
