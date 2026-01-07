use anyhow::Result;
use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Gauge, Histogram, Meter},
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
    // Transform-specific metrics
    pub collection_transform_jobs_total: Counter<u64>,
    pub collection_transform_files_processed: Counter<u64>,
    pub collection_transform_items_created: Counter<u64>,
    pub collection_transform_duration: Histogram<f64>,
    pub dataset_transform_jobs_total: Counter<u64>,
    pub dataset_transform_batches_processed: Counter<u64>,
    pub dataset_transform_chunks_embedded: Counter<u64>,
    pub dataset_transform_duration: Histogram<f64>,
    pub visualization_transform_jobs_total: Counter<u64>,
    pub visualization_transform_points_created: Counter<u64>,
    pub visualization_transform_clusters_created: Counter<u64>,
    pub visualization_transform_duration: Histogram<f64>,
    pub embedded_datasets_active: Gauge<f64>,
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

        // Collection Transform metrics
        let collection_transform_jobs_total = meter
            .u64_counter("collection_transform_jobs_total")
            .with_description("Total number of collection transform jobs processed")
            .build();

        let collection_transform_files_processed = meter
            .u64_counter("collection_transform_files_processed")
            .with_description("Total number of files processed by collection transforms")
            .build();

        let collection_transform_items_created = meter
            .u64_counter("collection_transform_items_created")
            .with_description("Total number of dataset items created by collection transforms")
            .build();

        let collection_transform_duration = meter
            .f64_histogram("collection_transform_duration_seconds")
            .with_description("Duration of collection transform jobs in seconds")
            .build();

        // Dataset Transform metrics
        let dataset_transform_jobs_total = meter
            .u64_counter("dataset_transform_jobs_total")
            .with_description("Total number of dataset transform jobs processed")
            .build();

        let dataset_transform_batches_processed = meter
            .u64_counter("dataset_transform_batches_processed")
            .with_description("Total number of batches processed by dataset transforms")
            .build();

        let dataset_transform_chunks_embedded = meter
            .u64_counter("dataset_transform_chunks_embedded")
            .with_description("Total number of chunks embedded by dataset transforms")
            .build();

        let dataset_transform_duration = meter
            .f64_histogram("dataset_transform_duration_seconds")
            .with_description("Duration of dataset transform jobs in seconds")
            .build();

        // Visualization Transform metrics
        let visualization_transform_jobs_total = meter
            .u64_counter("visualization_transform_jobs_total")
            .with_description("Total number of visualization transform jobs processed")
            .build();

        let visualization_transform_points_created = meter
            .u64_counter("visualization_transform_points_created")
            .with_description("Total number of visualization points created")
            .build();

        let visualization_transform_clusters_created = meter
            .u64_counter("visualization_transform_clusters_created")
            .with_description("Total number of clusters created by visualization transforms")
            .build();

        let visualization_transform_duration = meter
            .f64_histogram("visualization_transform_duration_seconds")
            .with_description("Duration of visualization transform jobs in seconds")
            .build();

        // Embedded Datasets gauge
        let embedded_datasets_active = meter
            .f64_gauge("embedded_datasets_active")
            .with_description("Number of active embedded datasets")
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
            collection_transform_jobs_total,
            collection_transform_files_processed,
            collection_transform_items_created,
            collection_transform_duration,
            dataset_transform_jobs_total,
            dataset_transform_batches_processed,
            dataset_transform_chunks_embedded,
            dataset_transform_duration,
            visualization_transform_jobs_total,
            visualization_transform_points_created,
            visualization_transform_clusters_created,
            visualization_transform_duration,
            embedded_datasets_active,
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

// Collection Transform metrics recording
pub fn record_collection_transform_job(
    transform_id: i32,
    duration_secs: f64,
    files_processed: u64,
    items_created: u64,
    status: &str,
) {
    let metrics = get_metrics();
    let transform_id_str = transform_id.to_string();

    metrics.collection_transform_jobs_total.add(
        1,
        &[
            KeyValue::new("transform_id", transform_id_str.clone()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.collection_transform_files_processed.add(
        files_processed,
        &[KeyValue::new("transform_id", transform_id_str.clone())],
    );

    metrics.collection_transform_items_created.add(
        items_created,
        &[KeyValue::new("transform_id", transform_id_str.clone())],
    );

    metrics.collection_transform_duration.record(
        duration_secs,
        &[
            KeyValue::new("transform_id", transform_id_str),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

// Dataset Transform metrics recording
pub fn record_dataset_transform_job(
    transform_id: i32,
    embedded_dataset_id: i32,
    duration_secs: f64,
    batches_processed: u64,
    chunks_embedded: u64,
    status: &str,
) {
    let metrics = get_metrics();
    let transform_id_str = transform_id.to_string();
    let embedded_dataset_id_str = embedded_dataset_id.to_string();

    metrics.dataset_transform_jobs_total.add(
        1,
        &[
            KeyValue::new("transform_id", transform_id_str.clone()),
            KeyValue::new("embedded_dataset_id", embedded_dataset_id_str.clone()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.dataset_transform_batches_processed.add(
        batches_processed,
        &[
            KeyValue::new("transform_id", transform_id_str.clone()),
            KeyValue::new("embedded_dataset_id", embedded_dataset_id_str.clone()),
        ],
    );

    metrics.dataset_transform_chunks_embedded.add(
        chunks_embedded,
        &[
            KeyValue::new("transform_id", transform_id_str.clone()),
            KeyValue::new("embedded_dataset_id", embedded_dataset_id_str.clone()),
        ],
    );

    metrics.dataset_transform_duration.record(
        duration_secs,
        &[
            KeyValue::new("transform_id", transform_id_str),
            KeyValue::new("embedded_dataset_id", embedded_dataset_id_str),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

// Visualization Transform metrics recording
pub fn record_visualization_transform_job(
    transform_id: i32,
    duration_secs: f64,
    points_created: u64,
    clusters_created: u64,
    status: &str,
) {
    let metrics = get_metrics();
    let transform_id_str = transform_id.to_string();

    metrics.visualization_transform_jobs_total.add(
        1,
        &[
            KeyValue::new("transform_id", transform_id_str.clone()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.visualization_transform_points_created.add(
        points_created,
        &[KeyValue::new("transform_id", transform_id_str.clone())],
    );

    metrics.visualization_transform_clusters_created.add(
        clusters_created,
        &[KeyValue::new("transform_id", transform_id_str.clone())],
    );

    metrics.visualization_transform_duration.record(
        duration_secs,
        &[
            KeyValue::new("transform_id", transform_id_str),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

// Embedded Datasets gauge update
pub fn update_embedded_datasets_count(count: u64) {
    let metrics = get_metrics();
    metrics.embedded_datasets_active.record(count as f64, &[]);
}
