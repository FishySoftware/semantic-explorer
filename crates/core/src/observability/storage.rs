use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_storage_operation(
    operation: &str,
    duration_secs: f64,
    file_size_bytes: Option<u64>,
    success: bool,
) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.storage_operations_total.add(
        1,
        &[
            KeyValue::new("operation", operation.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.storage_operation_duration.record(
        duration_secs,
        &[
            KeyValue::new("operation", operation.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if let Some(size) = file_size_bytes {
        metrics.storage_file_size_bytes.record(
            size as f64,
            &[KeyValue::new("operation", operation.to_string())],
        );
    }
}

pub fn record_storage_upload(
    bucket: &str,
    duration_secs: f64,
    size_bytes: Option<u64>,
    success: bool,
) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.storage_upload_duration.record(
        duration_secs,
        &[
            KeyValue::new("bucket", bucket.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if let Some(size) = size_bytes {
        metrics.storage_file_size_bytes.record(
            size as f64,
            &[
                KeyValue::new("operation", "upload"),
                KeyValue::new("bucket", bucket.to_string()),
            ],
        );
    }
}

pub fn record_storage_download(
    bucket: &str,
    duration_secs: f64,
    size_bytes: Option<u64>,
    success: bool,
) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.storage_download_duration.record(
        duration_secs,
        &[
            KeyValue::new("bucket", bucket.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if let Some(size) = size_bytes {
        metrics.storage_file_size_bytes.record(
            size as f64,
            &[
                KeyValue::new("operation", "download"),
                KeyValue::new("bucket", bucket.to_string()),
            ],
        );
    }
}

pub fn record_storage_delete(bucket: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.storage_delete_duration.record(
        duration_secs,
        &[
            KeyValue::new("bucket", bucket.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

pub fn record_storage_list(bucket: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.storage_list_duration.record(
        duration_secs,
        &[
            KeyValue::new("bucket", bucket.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}
