use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_valkey_cache_hit(cache_type: &str) {
    let metrics = get_metrics();
    metrics
        .valkey_cache_hits_total
        .add(1, &[KeyValue::new("cache_type", cache_type.to_string())]);
}

pub fn record_valkey_cache_miss(cache_type: &str) {
    let metrics = get_metrics();
    metrics
        .valkey_cache_misses_total
        .add(1, &[KeyValue::new("cache_type", cache_type.to_string())]);
}

pub fn record_valkey_cache_error(operation: &str) {
    let metrics = get_metrics();
    metrics
        .valkey_cache_errors_total
        .add(1, &[KeyValue::new("operation", operation.to_string())]);
}

pub fn record_valkey_operation(operation: &str, duration_secs: f64) {
    let metrics = get_metrics();
    metrics.valkey_operation_duration.record(
        duration_secs,
        &[KeyValue::new("operation", operation.to_string())],
    );
}

pub fn record_valkey_connected(connected: bool) {
    let metrics = get_metrics();
    metrics
        .valkey_connected
        .record(if connected { 1.0 } else { 0.0 }, &[]);
}

pub fn record_valkey_server_stats(
    used_memory: f64,
    connected_clients: f64,
    keyspace_hits: f64,
    keyspace_misses: f64,
) {
    let metrics = get_metrics();
    metrics.valkey_used_memory_bytes.record(used_memory, &[]);
    metrics
        .valkey_connected_clients
        .record(connected_clients, &[]);
    metrics.valkey_keyspace_hits.record(keyspace_hits, &[]);
    metrics.valkey_keyspace_misses.record(keyspace_misses, &[]);
}

pub fn record_bearer_l1_cache_hit() {
    let metrics = get_metrics();
    metrics.bearer_l1_cache_hits_total.add(1, &[]);
}

pub fn record_bearer_l1_cache_miss() {
    let metrics = get_metrics();
    metrics.bearer_l1_cache_misses_total.add(1, &[]);
}

pub fn record_bearer_l1_cache_size(entry_count: u64) {
    let metrics = get_metrics();
    metrics
        .bearer_l1_cache_entries
        .record(entry_count as f64, &[]);
}
