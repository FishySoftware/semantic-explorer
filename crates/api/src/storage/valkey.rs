//! Valkey (Redis-compatible) cache client initialization and helpers.
//!
//! Provides a shared caching layer across all API replicas for:
//! - Bearer token → user info lookups (replacing the PostgreSQL L2 cache)
//! - Resource metadata cache-aside (collections, datasets, embedders, LLMs)
//!
//! All operations are fallible and return `None` / silently fail on connection
//! errors, providing graceful degradation when Valkey is unavailable.

use std::time::Instant;

use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Client};
use semantic_explorer_core::config::ValkeyConfig;
use serde::{Serialize, de::DeserializeOwned};
use tracing::{debug, warn};

use crate::observability::valkey_metrics;

/// Wrapper holding both write (primary) and read (replica) Valkey connections.
/// The read connection falls back to the primary if no read replica is configured.
#[derive(Clone)]
pub struct ValkeyClients {
    /// Write client — connects to the Valkey primary for SET operations.
    pub write: ConnectionManager,
    /// Read client — connects to a read replica (or primary if no replica configured).
    pub read: ConnectionManager,
}

/// Initialize the Valkey write client (primary) connection manager.
///
/// The `ConnectionManager` provides automatic reconnection on failures
/// and is cheaply cloneable (backed by `Arc`).
pub(crate) async fn initialize_client(config: &ValkeyConfig) -> Result<ConnectionManager> {
    let url = build_url(&config.url, config.password.as_deref(), config.tls_enabled);
    let client = Client::open(url.as_str())
        .with_context(|| format!("Failed to create Valkey client for URL: {}", config.url))?;

    let manager = ConnectionManager::new(client)
        .await
        .context("Failed to connect to Valkey")?;

    // Verify connectivity with a PING
    let mut conn = manager.clone();
    let pong: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .context("Valkey PING failed — is the server reachable?")?;
    debug!("Valkey primary connected (PING → {pong})");

    Ok(manager)
}

/// Initialize the Valkey read client (read-replica) connection manager.
/// Falls back to the primary URL if `VALKEY_READ_URL` is not set separately.
pub(crate) async fn initialize_read_client(config: &ValkeyConfig) -> Result<ConnectionManager> {
    let url = build_url(
        &config.read_url,
        config.password.as_deref(),
        config.tls_enabled,
    );
    let client = Client::open(url.as_str()).with_context(|| {
        format!(
            "Failed to create Valkey read client for URL: {}",
            config.read_url
        )
    })?;

    let manager = ConnectionManager::new(client)
        .await
        .context("Failed to connect to Valkey read replica")?;

    let mut conn = manager.clone();
    let pong: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .context("Valkey read replica PING failed")?;
    debug!("Valkey read replica connected (PING → {pong})");

    Ok(manager)
}

/// Build a redis:// or rediss:// URL with optional password.
fn build_url(base_url: &str, password: Option<&str>, tls: bool) -> String {
    let scheme = if tls { "rediss" } else { "redis" };
    // If the URL already has a scheme, strip it so we can reconstruct
    let host_port = base_url
        .trim_start_matches("redis://")
        .trim_start_matches("rediss://");

    match password {
        Some(pw) => format!("{scheme}://default:{pw}@{host_port}"),
        None => format!("{scheme}://{host_port}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cache helpers — all operations are fallible with graceful degradation
// ─────────────────────────────────────────────────────────────────────────────

/// Store a serializable value in Valkey with a TTL.
/// Returns silently on connection errors (cache is best-effort).
pub(crate) async fn cache_set<T: Serialize>(
    conn: &ConnectionManager,
    key: &str,
    value: &T,
    ttl_secs: u64,
) {
    let start = Instant::now();
    let json = match serde_json::to_string(value) {
        Ok(j) => j,
        Err(e) => {
            warn!(error = %e, key, "Failed to serialize value for Valkey cache");
            valkey_metrics::record_error("SET");
            return;
        }
    };

    let mut conn = conn.clone();
    let result: Result<(), redis::RedisError> = conn.set_ex(key, &json, ttl_secs).await;

    let duration = start.elapsed();
    match result {
        Ok(()) => {
            valkey_metrics::record_operation("SET", duration);
            debug!(key, ttl_secs, "Valkey SET OK");
        }
        Err(e) => {
            warn!(error = %e, key, "Valkey SET failed (graceful degradation)");
            valkey_metrics::record_error("SET");
        }
    }
}

/// Retrieve a cached value from Valkey. Returns `None` on miss or error.
pub(crate) async fn cache_get<T: DeserializeOwned>(
    conn: &ConnectionManager,
    key: &str,
) -> Option<T> {
    let start = Instant::now();
    let mut conn = conn.clone();
    let result: Result<Option<String>, redis::RedisError> = conn.get(key).await;

    let duration = start.elapsed();
    match result {
        Ok(Some(json)) => match serde_json::from_str::<T>(&json) {
            Ok(value) => {
                valkey_metrics::record_hit(key);
                valkey_metrics::record_operation("GET", duration);
                Some(value)
            }
            Err(e) => {
                warn!(error = %e, key, "Valkey deserialization error");
                valkey_metrics::record_error("GET");
                None
            }
        },
        Ok(None) => {
            valkey_metrics::record_miss(key);
            valkey_metrics::record_operation("GET", duration);
            None
        }
        Err(e) => {
            warn!(error = %e, key, "Valkey GET failed (graceful degradation)");
            valkey_metrics::record_error("GET");
            None
        }
    }
}

/// Check Valkey health by sending a PING command.
/// Returns `true` if the server responds with PONG.
pub(crate) async fn health_check(conn: &ConnectionManager) -> bool {
    let mut conn = conn.clone();
    let result: Result<String, redis::RedisError> = redis::cmd("PING").query_async(&mut conn).await;
    result.map(|r| r == "PONG").unwrap_or(false)
}

/// Get Valkey server info (used for metrics/monitoring).
pub(crate) async fn get_info(conn: &ConnectionManager) -> Option<String> {
    let mut conn = conn.clone();
    redis::cmd("INFO")
        .query_async::<Option<String>>(&mut conn)
        .await
        .ok()
        .flatten()
}

/// Parse a specific field from Valkey INFO output.
pub(crate) fn parse_info_field(info: &str, field: &str) -> Option<f64> {
    info.lines()
        .find(|line| line.starts_with(&format!("{field}:")))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|val| val.trim().parse::<f64>().ok())
}

/// Extract the cache type label from a cache key for metrics.
/// e.g., "bearer:abc123" → "bearer", "collection:uuid" → "collection"
fn _cache_type_from_key(key: &str) -> &str {
    key.split(':').next().unwrap_or("unknown")
}

/// Delete all keys matching a prefix pattern using SCAN + DEL.
/// This avoids the O(N) KEYS command and is safe for production.
/// Returns the number of keys deleted, or 0 on error.
pub(crate) async fn cache_del_by_prefix(conn: &ConnectionManager, prefix: &str) -> usize {
    let start = Instant::now();
    let mut conn = conn.clone();
    let pattern = format!("{prefix}*");
    let mut deleted = 0usize;

    // Use SCAN to find matching keys in batches
    let mut cursor = 0u64;
    loop {
        let result: Result<(u64, Vec<String>), redis::RedisError> = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg(100)
            .query_async(&mut conn)
            .await;

        match result {
            Ok((next_cursor, keys)) => {
                if !keys.is_empty() {
                    let del_result: Result<(), redis::RedisError> = conn.del::<_, ()>(&keys).await;
                    if del_result.is_ok() {
                        deleted += keys.len();
                    }
                }
                cursor = next_cursor;
                if cursor == 0 {
                    break;
                }
            }
            Err(e) => {
                warn!(error = %e, prefix, "Valkey SCAN failed during prefix delete");
                valkey_metrics::record_error("DEL_PREFIX");
                break;
            }
        }
    }

    let duration = start.elapsed();
    valkey_metrics::record_operation("DEL_PREFIX", duration);
    debug!(prefix, deleted, "Valkey DEL_PREFIX complete");
    deleted
}

/// Invalidate all cached listing pages for a resource type and owner.
///
/// Call this after any create/update/delete mutation to keep the cache
/// consistent with the database. The invalidation runs asynchronously
/// (fire-and-forget) so it never blocks the HTTP response.
///
/// `resource_type` should be one of: "datasets", "collections", "embedders", "llms"
pub(crate) fn invalidate_resource_cache(
    valkey: Option<&actix_web::web::Data<ValkeyClients>>,
    resource_type: &str,
    owner_id: &str,
) {
    if let Some(v) = valkey {
        let conn = v.write.clone();
        let prefix = format!("{resource_type}:{owner_id}:");
        actix_web::rt::spawn(async move {
            cache_del_by_prefix(&conn, &prefix).await;
        });
    }
}
