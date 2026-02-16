//! Valkey cache metrics helpers.
//!
//! Thin wrappers around `semantic_explorer_core::observability::record_valkey_*`
//! that extract cache-type labels from key prefixes.

use std::time::Duration;

/// Extract cache type from a key prefix (e.g., "bearer:abc" → "bearer")
fn cache_type_from_key(key: &str) -> &str {
    key.split(':').next().unwrap_or("unknown")
}

/// Record a cache hit, extracting the cache type from the key prefix.
pub(crate) fn record_hit(key: &str) {
    let cache_type = cache_type_from_key(key);
    semantic_explorer_core::observability::record_valkey_cache_hit(cache_type);
}

/// Record a cache miss, extracting the cache type from the key prefix.
pub(crate) fn record_miss(key: &str) {
    let cache_type = cache_type_from_key(key);
    semantic_explorer_core::observability::record_valkey_cache_miss(cache_type);
}

/// Record a cache error for an operation.
pub(crate) fn record_error(operation: &str) {
    semantic_explorer_core::observability::record_valkey_cache_error(operation);
}

/// Record a cache operation duration.
pub(crate) fn record_operation(operation: &str, duration: Duration) {
    semantic_explorer_core::observability::record_valkey_operation(
        operation,
        duration.as_secs_f64(),
    );
}
