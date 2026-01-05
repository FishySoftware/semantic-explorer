use anyhow::Result;
use opentelemetry::{
    global,
    metrics::{Counter, Gauge, Histogram, Meter},
    KeyValue,
};
use std::sync::OnceLock;

pub struct Metrics {
    pub database_query_total: Counter<u64>,
    pub database_query_duration: Histogram<f64>,
    pub database_connection_pool_size: Gauge<f64>,
    pub database_connection_pool_active: Gauge<f64>,
    pub database_connection_pool_idle: Gauge<f64>,
    pub storage_operations_total: Counter<u64>,
    pub storage_operation_duration: Histogram<f64>,
    pub storage_file_size_bytes: Histogram<f64>,
    pub worker_jobs_total: Counter<u64>,
    pub worker_job_duration: Histogram<f64>,
    pub worker_job_chunks: Histogram<f64>,
    pub worker_job_file_size: Histogram<f64>,
}

impl Metrics {
    fn new(meter: Meter) -> Self {
        let database_query_total = meter
            .u64_counter("database_query_total")
            .with_description("Total number of database queries")
            .build();

        let database_query_duration = meter
            .f64_histogram("database_query_duration_seconds")
            .with_description("Duration of database queries in seconds")
            .build();

        let database_connection_pool_size = meter
            .f64_gauge("database_connection_pool_size")
            .with_description("Total size of the database connection pool")
            .build();

        let database_connection_pool_active = meter
            .f64_gauge("database_connection_pool_active")
            .with_description("Number of active database connections")
            .build();

        let database_connection_pool_idle = meter
            .f64_gauge("database_connection_pool_idle")
            .with_description("Number of idle database connections")
            .build();

        let storage_operations_total = meter
            .u64_counter("storage_operations_total")
            .with_description("Total number of storage operations")
            .build();

        let storage_operation_duration = meter
            .f64_histogram("storage_operation_duration_seconds")
            .with_description("Duration of storage operations in seconds")
            .build();

        let storage_file_size_bytes = meter
            .f64_histogram("storage_file_size_bytes")
            .with_description("Size of files in storage operations")
            .build();

        let worker_jobs_total = meter
            .u64_counter("worker_jobs_total")
            .with_description("Total number of worker jobs processed")
            .build();

        let worker_job_duration = meter
            .f64_histogram("worker_job_duration_seconds")
            .with_description("Duration of worker jobs in seconds")
            .build();

        let worker_job_chunks = meter
            .f64_histogram("worker_job_chunks")
            .with_description("Number of chunks processed in worker jobs")
            .build();

        let worker_job_file_size = meter
            .f64_histogram("worker_job_file_size_bytes")
            .with_description("Size of files processed in worker jobs")
            .build();

        Self {
            database_query_total,
            database_query_duration,
            database_connection_pool_size,
            database_connection_pool_active,
            database_connection_pool_idle,
            storage_operations_total,
            storage_operation_duration,
            storage_file_size_bytes,
            worker_jobs_total,
            worker_job_duration,
            worker_job_chunks,
            worker_job_file_size,
        }
    }
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

pub fn init_metrics_otel() -> Result<()> {
    let meter = global::meter("semantic-explorer");
    let metrics = Metrics::new(meter);
    METRICS
        .set(metrics)
        .map_err(|_| anyhow::anyhow!("Metrics already initialized"))?;
    Ok(())
}

pub fn get_metrics() -> &'static Metrics {
    METRICS.get().expect("Metrics not initialized")
}

pub fn record_database_query(operation: &str, table: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.database_query_total.add(
        1,
        &[
            KeyValue::new("operation", operation.to_string()),
            KeyValue::new("table", table.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.database_query_duration.record(
        duration_secs,
        &[
            KeyValue::new("operation", operation.to_string()),
            KeyValue::new("table", table.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

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

pub fn update_database_pool_stats(size: u64, active: u64, idle: u64) {
    let metrics = get_metrics();

    metrics
        .database_connection_pool_size
        .record(size as f64, &[]);

    metrics
        .database_connection_pool_active
        .record(active as f64, &[]);

    metrics
        .database_connection_pool_idle
        .record(idle as f64, &[]);
}
