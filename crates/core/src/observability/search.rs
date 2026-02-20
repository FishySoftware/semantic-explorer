use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_search_request(
    duration_secs: f64,
    embedder_duration_secs: f64,
    qdrant_duration_secs: f64,
    results_count: usize,
    embedded_datasets_count: usize,
    status: &str,
) {
    let metrics = get_metrics();

    metrics.search_request_total.add(
        1,
        &[
            KeyValue::new("status", status.to_string()),
            KeyValue::new("embedded_datasets", embedded_datasets_count.to_string()),
        ],
    );

    metrics.search_request_duration.record(
        duration_secs,
        &[
            KeyValue::new("status", status.to_string()),
            KeyValue::new("embedded_datasets", embedded_datasets_count.to_string()),
        ],
    );

    metrics.search_embedder_call_duration.record(
        embedder_duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );

    metrics.search_qdrant_query_duration.record(
        qdrant_duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );

    metrics.search_results_returned.record(
        results_count as f64,
        &[
            KeyValue::new("status", status.to_string()),
            KeyValue::new("embedded_datasets", embedded_datasets_count.to_string()),
        ],
    );
}
