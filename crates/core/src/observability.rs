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
    // NATS queue depth metrics
    pub nats_stream_messages: Gauge<f64>,
    pub nats_consumer_pending: Gauge<f64>,
    pub nats_consumer_ack_pending: Gauge<f64>,
    pub nats_stream_bytes: Gauge<f64>,
    // NATS latency metrics
    pub nats_publish_duration: Histogram<f64>,
    pub nats_subscribe_latency: Histogram<f64>,
    // Search performance metrics
    pub search_request_total: Counter<u64>,
    pub search_request_duration: Histogram<f64>,
    pub search_embedder_call_duration: Histogram<f64>,
    pub search_qdrant_query_duration: Histogram<f64>,
    pub search_results_returned: Histogram<f64>,
    // HTTP request metrics
    pub http_requests_total: Counter<u64>,
    pub http_request_duration: Histogram<f64>,
    pub http_requests_in_flight: Gauge<f64>,
    // Server-Sent Events metrics
    pub sse_connections_active: Gauge<f64>,
    pub sse_messages_sent: Counter<u64>,
    pub sse_connection_duration: Histogram<f64>,
    // Worker job failure tracking
    pub worker_job_failures_total: Counter<u64>,
    pub worker_job_retries_total: Counter<u64>,
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

        // NATS queue depth metrics
        let nats_stream_messages = meter
            .f64_gauge("nats_stream_messages")
            .with_description("Number of messages in NATS stream")
            .build();

        let nats_consumer_pending = meter
            .f64_gauge("nats_consumer_pending")
            .with_description("Number of pending messages for NATS consumer")
            .build();

        let nats_consumer_ack_pending = meter
            .f64_gauge("nats_consumer_ack_pending")
            .with_description("Number of messages pending acknowledgement")
            .build();

        let nats_stream_bytes = meter
            .f64_gauge("nats_stream_bytes")
            .with_description("Size of NATS stream in bytes")
            .build();

        // NATS latency metrics
        let nats_publish_duration = meter
            .f64_histogram("nats_publish_duration_seconds")
            .with_description("Duration of NATS message publish operations in seconds")
            .build();

        let nats_subscribe_latency = meter
            .f64_histogram("nats_subscribe_latency_seconds")
            .with_description("Latency between NATS message publish and processing in seconds")
            .build();

        // Search performance metrics
        let search_request_total = meter
            .u64_counter("search_request_total")
            .with_description("Total number of search requests")
            .build();

        let search_request_duration = meter
            .f64_histogram("search_request_duration_seconds")
            .with_description("Total duration of search requests in seconds")
            .build();

        let search_embedder_call_duration = meter
            .f64_histogram("search_embedder_call_duration_seconds")
            .with_description("Duration of embedder calls during search")
            .build();

        let search_qdrant_query_duration = meter
            .f64_histogram("search_qdrant_query_duration_seconds")
            .with_description("Duration of Qdrant queries during search")
            .build();

        let search_results_returned = meter
            .f64_histogram("search_results_returned")
            .with_description("Number of results returned per search")
            .build();

        // HTTP request metrics
        let http_requests_total = meter
            .u64_counter("http_requests_total")
            .with_description("Total number of HTTP requests")
            .build();

        let http_request_duration = meter
            .f64_histogram("http_request_duration_seconds")
            .with_description("Duration of HTTP requests in seconds")
            .build();

        let http_requests_in_flight = meter
            .f64_gauge("http_requests_in_flight")
            .with_description("Number of HTTP requests currently being processed")
            .build();

        // Server-Sent Events metrics
        let sse_connections_active = meter
            .f64_gauge("sse_connections_active")
            .with_description("Number of active Server-Sent Events connections")
            .build();

        let sse_messages_sent = meter
            .u64_counter("sse_messages_sent")
            .with_description("Total number of messages sent via Server-Sent Events")
            .build();

        let sse_connection_duration = meter
            .f64_histogram("sse_connection_duration_seconds")
            .with_description("Duration of Server-Sent Events connections in seconds")
            .build();

        // Worker job failure tracking
        let worker_job_failures_total = meter
            .u64_counter("worker_job_failures_total")
            .with_description("Total number of worker job failures")
            .build();

        let worker_job_retries_total = meter
            .u64_counter("worker_job_retries_total")
            .with_description("Total number of worker job retries")
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
            nats_stream_messages,
            nats_consumer_pending,
            nats_consumer_ack_pending,
            nats_stream_bytes,
            nats_publish_duration,
            nats_subscribe_latency,
            search_request_total,
            search_request_duration,
            search_embedder_call_duration,
            search_qdrant_query_duration,
            search_results_returned,
            http_requests_total,
            http_request_duration,
            http_requests_in_flight,
            sse_connections_active,
            sse_messages_sent,
            sse_connection_duration,
            worker_job_failures_total,
            worker_job_retries_total,
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

// NATS queue metrics
pub fn update_nats_stream_stats(stream_name: &str, messages: u64, bytes: u64) {
    let metrics = get_metrics();
    metrics.nats_stream_messages.record(
        messages as f64,
        &[KeyValue::new("stream", stream_name.to_string())],
    );
    metrics.nats_stream_bytes.record(
        bytes as f64,
        &[KeyValue::new("stream", stream_name.to_string())],
    );
}

pub fn update_nats_consumer_stats(
    stream_name: &str,
    consumer_name: &str,
    pending: u64,
    ack_pending: u64,
) {
    let metrics = get_metrics();
    metrics.nats_consumer_pending.record(
        pending as f64,
        &[
            KeyValue::new("stream", stream_name.to_string()),
            KeyValue::new("consumer", consumer_name.to_string()),
        ],
    );
    metrics.nats_consumer_ack_pending.record(
        ack_pending as f64,
        &[
            KeyValue::new("stream", stream_name.to_string()),
            KeyValue::new("consumer", consumer_name.to_string()),
        ],
    );
}

// Search metrics
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

// HTTP request metrics
pub fn record_http_request(method: &str, path: &str, status_code: u16, duration_secs: f64) {
    let metrics = get_metrics();

    metrics.http_requests_total.add(
        1,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("path", path.to_string()),
            KeyValue::new("status", status_code.to_string()),
        ],
    );

    metrics.http_request_duration.record(
        duration_secs,
        &[
            KeyValue::new("method", method.to_string()),
            KeyValue::new("path", path.to_string()),
            KeyValue::new("status", status_code.to_string()),
        ],
    );
}

pub fn increment_http_requests_in_flight(path: &str) {
    let metrics = get_metrics();
    metrics
        .http_requests_in_flight
        .record(1.0, &[KeyValue::new("path", path.to_string())]);
}

pub fn decrement_http_requests_in_flight(path: &str) {
    let metrics = get_metrics();
    metrics
        .http_requests_in_flight
        .record(-1.0, &[KeyValue::new("path", path.to_string())]);
}

// Worker failure tracking
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
