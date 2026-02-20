use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_document_upload(operation: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.document_upload_per_item_duration.record(
        duration_secs,
        &[
            KeyValue::new("operation", operation.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

pub fn record_document_extraction(operation: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.document_extraction_duration.record(
        duration_secs,
        &[
            KeyValue::new("operation", operation.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

pub fn record_document_chunking(operation: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.document_chunking_per_item_duration.record(
        duration_secs,
        &[
            KeyValue::new("operation", operation.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

pub fn record_chat_request(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.chat_request_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

pub fn record_visualization_fetch_vectors(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_fetch_vectors_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

pub fn record_visualization_umap(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_umap_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

pub fn record_visualization_hdbscan(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_hdbscan_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

pub fn record_visualization_plot(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_plot_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}
