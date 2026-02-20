use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_scanner_trigger_published(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_triggers_published_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

pub fn record_scanner_trigger_processed(scanner_type: &str, success: bool) {
    let metrics = get_metrics();
    metrics.scanner_triggers_processed_total.add(
        1,
        &[
            KeyValue::new("scanner_type", scanner_type.to_string()),
            KeyValue::new("success", success.to_string()),
        ],
    );
}

pub fn record_scanner_items_discovered(scanner_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_items_discovered_total.add(
        count,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

pub fn record_scanner_scan_duration(scanner_type: &str, duration_secs: f64) {
    let metrics = get_metrics();
    metrics.scanner_scan_duration.record(
        duration_secs,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

pub fn record_scanner_backpressure_skip(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_backpressure_skips_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

pub fn record_scanner_failed_batch_recovery(scanner_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_failed_batch_recoveries_total.add(
        count,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

pub fn record_scanner_orphaned_batch_cleanup(count: u64) {
    let metrics = get_metrics();
    metrics
        .scanner_orphaned_batch_cleanups_total
        .add(count, &[]);
}

pub fn record_scanner_pending_batch_recovery(batch_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_pending_batch_recoveries_total.add(
        count,
        &[KeyValue::new("batch_type", batch_type.to_string())],
    );
}

pub fn record_scanner_circuit_breaker_trip(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_circuit_breaker_trips_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

pub fn record_scanner_batches_created(scanner_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_batches_created_total.add(
        count,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

pub fn record_scanner_stats_refresh_skip(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_stats_refresh_skips_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}
