use opentelemetry::KeyValue;

use super::get_metrics;

pub fn record_worker_job(worker: &str, duration_secs: f64, status: &str) {
    let metrics = get_metrics();

    metrics.worker_jobs_total.add(
        1,
        &[
            KeyValue::new("worker", worker.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.worker_job_duration.record(
        duration_secs,
        &[
            KeyValue::new("worker", worker.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

pub fn record_worker_job_with_metrics(
    worker: &str,
    duration_secs: f64,
    status: &str,
    chunk_count: Option<usize>,
    file_size_bytes: Option<u64>,
) {
    let metrics = get_metrics();

    metrics.worker_jobs_total.add(
        1,
        &[
            KeyValue::new("worker", worker.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.worker_job_duration.record(
        duration_secs,
        &[
            KeyValue::new("worker", worker.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if let Some(chunks) = chunk_count {
        metrics.worker_job_chunks.record(
            chunks as f64,
            &[
                KeyValue::new("worker", worker.to_string()),
                KeyValue::new("status", status.to_string()),
            ],
        );
    }

    if let Some(size) = file_size_bytes {
        metrics.worker_job_file_size.record(
            size as f64,
            &[
                KeyValue::new("worker", worker.to_string()),
                KeyValue::new("status", status.to_string()),
            ],
        );
    }
}

pub fn set_worker_ready(worker: &str, ready: bool) {
    let metrics = get_metrics();
    let value = if ready { 1.0 } else { 0.0 };

    metrics
        .worker_ready
        .record(value, &[KeyValue::new("worker", worker.to_string())]);
}

pub fn record_worker_job_failure(worker: &str, error_type: &str) {
    let metrics = get_metrics();
    metrics.worker_job_failures_total.add(
        1,
        &[
            KeyValue::new("worker", worker.to_string()),
            KeyValue::new("error_type", error_type.to_string()),
        ],
    );
}

pub fn record_worker_job_retry(worker: &str, attempt: u32) {
    let metrics = get_metrics();
    metrics.worker_job_retries_total.add(
        1,
        &[
            KeyValue::new("worker", worker.to_string()),
            KeyValue::new("attempt", attempt.to_string()),
        ],
    );
}
