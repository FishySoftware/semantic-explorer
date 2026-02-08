use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use anyhow::Result;
use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Gauge, Histogram, Meter},
    trace::TracerProvider,
};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    logs::SdkLoggerProvider,
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use std::{sync::OnceLock, time::Duration};
use tracing_subscriber::{
    EnvFilter, Layer, Registry, layer::SubscriberExt, util::SubscriberInitExt,
};

pub struct Metrics {
    pub database_query_total: Counter<u64>,
    pub database_query_duration: Histogram<f64>,
    pub database_connection_pool_size: Gauge<f64>,
    pub database_connection_pool_active: Gauge<f64>,
    pub database_connection_pool_idle: Gauge<f64>,
    pub storage_operations_total: Counter<u64>,
    pub storage_operation_duration: Histogram<f64>,
    pub storage_file_size_bytes: Histogram<f64>,
    pub worker_ready: Gauge<f64>,
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
    // Inference API metrics
    pub inference_embed_requests_total: Counter<u64>,
    pub inference_embed_duration: Histogram<f64>,
    pub inference_embed_items_total: Counter<u64>,
    pub inference_embed_per_item_duration: Histogram<f64>,
    pub inference_rerank_requests_total: Counter<u64>,
    pub inference_rerank_duration: Histogram<f64>,
    pub inference_rerank_documents_total: Counter<u64>,
    // LLM inference metrics
    pub inference_llm_requests_total: Counter<u64>,
    pub inference_llm_duration: Histogram<f64>,
    pub inference_llm_tokens_generated: Counter<u64>,
    pub inference_llm_tokens_per_second: Histogram<f64>,
    // Granular operation timing metrics
    pub document_upload_per_item_duration: Histogram<f64>,
    pub document_extraction_duration: Histogram<f64>,
    pub document_chunking_per_item_duration: Histogram<f64>,
    pub embedding_per_chunk_duration: Histogram<f64>,
    pub llm_response_duration: Histogram<f64>,
    pub chat_request_duration: Histogram<f64>,
    pub visualization_fetch_vectors_duration: Histogram<f64>,
    pub visualization_umap_duration: Histogram<f64>,
    pub visualization_hdbscan_duration: Histogram<f64>,
    pub visualization_plot_duration: Histogram<f64>,
    pub storage_upload_duration: Histogram<f64>,
    pub storage_download_duration: Histogram<f64>,
    pub storage_delete_duration: Histogram<f64>,
    pub storage_list_duration: Histogram<f64>,
    // GPU metrics for VRAM monitoring
    pub gpu_memory_used_bytes: Gauge<f64>,
    pub gpu_memory_total_bytes: Gauge<f64>,
    pub gpu_utilization_percent: Gauge<f64>,
    pub gpu_memory_utilization_percent: Gauge<f64>,
    // Embedding session lifecycle metrics
    pub embedding_session_resets_total: Counter<u64>,
    pub embedding_session_request_count: Gauge<f64>,
    pub embedding_session_age_seconds: Gauge<f64>,
    // Dead Letter Queue metrics
    pub dlq_messages_total: Counter<u64>,
    // Scanner trigger metrics
    pub scanner_triggers_published_total: Counter<u64>,
    pub scanner_triggers_processed_total: Counter<u64>,
    pub scanner_items_discovered_total: Counter<u64>,
    pub scanner_scan_duration: Histogram<f64>,
    // Scanner resilience metrics (#10)
    pub scanner_backpressure_skips_total: Counter<u64>,
    pub scanner_failed_batch_recoveries_total: Counter<u64>,
    pub scanner_orphaned_batch_cleanups_total: Counter<u64>,
    pub scanner_pending_batch_recoveries_total: Counter<u64>,
    pub scanner_circuit_breaker_trips_total: Counter<u64>,
    pub scanner_batches_created_total: Counter<u64>,
    pub scanner_stats_refresh_skips_total: Counter<u64>,
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

        let worker_ready = meter
            .f64_gauge("worker_ready")
            .with_description("Worker ready status (1 = ready, 0 = not ready)")
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

        // Inference API metrics
        let inference_embed_requests_total = meter
            .u64_counter("inference_embed_requests_total")
            .with_description("Total number of embedding requests")
            .build();

        let inference_embed_duration = meter
            .f64_histogram("inference_embed_duration_seconds")
            .with_description("Duration of embedding requests in seconds")
            .build();

        let inference_embed_items_total = meter
            .u64_counter("inference_embed_items_total")
            .with_description("Total number of items embedded")
            .build();

        let inference_embed_per_item_duration = meter
            .f64_histogram("inference_embed_per_item_duration_seconds")
            .with_description("Average duration per item in embedding requests")
            .build();

        let inference_rerank_requests_total = meter
            .u64_counter("inference_rerank_requests_total")
            .with_description("Total number of reranking requests")
            .build();

        let inference_rerank_duration = meter
            .f64_histogram("inference_rerank_duration_seconds")
            .with_description("Duration of reranking requests in seconds")
            .build();

        let inference_rerank_documents_total = meter
            .u64_counter("inference_rerank_documents_total")
            .with_description("Total number of documents reranked")
            .build();

        let inference_llm_requests_total = meter
            .u64_counter("inference_llm_requests_total")
            .with_description("Total number of LLM generation requests")
            .build();

        let inference_llm_duration = meter
            .f64_histogram("inference_llm_duration_seconds")
            .with_description("Duration of LLM generation requests in seconds")
            .build();

        let inference_llm_tokens_generated = meter
            .u64_counter("inference_llm_tokens_generated_total")
            .with_description("Total number of tokens generated by LLMs")
            .build();

        let inference_llm_tokens_per_second = meter
            .f64_histogram("inference_llm_tokens_per_second")
            .with_description("Tokens generated per second (throughput)")
            .build();

        // Granular operation timing metrics
        let document_upload_per_item_duration = meter
            .f64_histogram("document_upload_per_item_duration_seconds")
            .with_description("Duration to upload individual documents in seconds")
            .build();

        let document_extraction_duration = meter
            .f64_histogram("document_extraction_duration_seconds")
            .with_description("Duration to extract text from documents in seconds")
            .build();

        let document_chunking_per_item_duration = meter
            .f64_histogram("document_chunking_per_item_duration_seconds")
            .with_description("Duration to chunk individual documents in seconds")
            .build();

        let embedding_per_chunk_duration = meter
            .f64_histogram("embedding_per_chunk_duration_seconds")
            .with_description("Duration to generate embeddings per chunk in seconds")
            .build();

        let llm_response_duration = meter
            .f64_histogram("llm_response_duration_seconds")
            .with_description("Duration to generate LLM responses in seconds")
            .build();

        let chat_request_duration = meter
            .f64_histogram("chat_request_duration_seconds")
            .with_description("Duration of chat requests in seconds")
            .build();

        let visualization_fetch_vectors_duration = meter
            .f64_histogram("visualization_fetch_vectors_duration_seconds")
            .with_description("Duration to fetch vectors for visualization in seconds")
            .build();

        let visualization_umap_duration = meter
            .f64_histogram("visualization_umap_duration_seconds")
            .with_description("Duration to run UMAP dimensionality reduction in seconds")
            .build();

        let visualization_hdbscan_duration = meter
            .f64_histogram("visualization_hdbscan_duration_seconds")
            .with_description("Duration to run HDBSCAN clustering in seconds")
            .build();

        let visualization_plot_duration = meter
            .f64_histogram("visualization_plot_duration_seconds")
            .with_description("Duration to generate visualization plots in seconds")
            .build();

        let storage_upload_duration = meter
            .f64_histogram("storage_upload_duration_seconds")
            .with_description("Duration of S3 upload operations in seconds")
            .build();

        let storage_download_duration = meter
            .f64_histogram("storage_download_duration_seconds")
            .with_description("Duration of S3 download operations in seconds")
            .build();

        let storage_delete_duration = meter
            .f64_histogram("storage_delete_duration_seconds")
            .with_description("Duration of S3 delete operations in seconds")
            .build();

        let storage_list_duration = meter
            .f64_histogram("storage_list_duration_seconds")
            .with_description("Duration of S3 list operations in seconds")
            .build();

        // GPU metrics for VRAM monitoring
        let gpu_memory_used_bytes = meter
            .f64_gauge("gpu_memory_used_bytes")
            .with_description("GPU memory currently used in bytes")
            .build();

        let gpu_memory_total_bytes = meter
            .f64_gauge("gpu_memory_total_bytes")
            .with_description("Total GPU memory available in bytes")
            .build();

        let gpu_utilization_percent = meter
            .f64_gauge("gpu_utilization_percent")
            .with_description("GPU compute utilization percentage")
            .build();

        let gpu_memory_utilization_percent = meter
            .f64_gauge("gpu_memory_utilization_percent")
            .with_description("GPU memory utilization percentage")
            .build();

        // Embedding session lifecycle metrics
        let embedding_session_resets_total = meter
            .u64_counter("embedding_session_resets_total")
            .with_description("Total number of embedding session resets due to memory thresholds")
            .build();

        let embedding_session_request_count = meter
            .f64_gauge("embedding_session_request_count")
            .with_description("Current request count for each embedding model session")
            .build();

        let embedding_session_age_seconds = meter
            .f64_gauge("embedding_session_age_seconds")
            .with_description("Age of the current embedding model session in seconds")
            .build();

        let dlq_messages_total = meter
            .u64_counter("dlq_messages_total")
            .with_description("Total number of messages sent to Dead Letter Queue")
            .build();

        let scanner_triggers_published_total = meter
            .u64_counter("scanner_triggers_published_total")
            .with_description("Total number of scanner triggers published")
            .build();

        let scanner_triggers_processed_total = meter
            .u64_counter("scanner_triggers_processed_total")
            .with_description("Total number of scanner triggers processed")
            .build();

        let scanner_items_discovered_total = meter
            .u64_counter("scanner_items_discovered_total")
            .with_description("Total number of items discovered by scanners")
            .build();

        let scanner_scan_duration = meter
            .f64_histogram("scanner_scan_duration_seconds")
            .with_description("Duration of scanner scans in seconds")
            .build();

        // Scanner resilience metrics (#10)
        let scanner_backpressure_skips_total = meter
            .u64_counter("scanner_backpressure_skips_total")
            .with_description("Total number of scans skipped due to backpressure")
            .build();

        let scanner_failed_batch_recoveries_total = meter
            .u64_counter("scanner_failed_batch_recoveries_total")
            .with_description("Total number of failed batches recovered by reconciliation")
            .build();

        let scanner_orphaned_batch_cleanups_total = meter
            .u64_counter("scanner_orphaned_batch_cleanups_total")
            .with_description("Total number of orphaned batches cleaned up")
            .build();

        let scanner_pending_batch_recoveries_total = meter
            .u64_counter("scanner_pending_batch_recoveries_total")
            .with_description("Total number of pending batches recovered")
            .build();

        let scanner_circuit_breaker_trips_total = meter
            .u64_counter("scanner_circuit_breaker_trips_total")
            .with_description("Total number of circuit breaker trips in scanner")
            .build();

        let scanner_batches_created_total = meter
            .u64_counter("scanner_batches_created_total")
            .with_description("Total number of batches created by scanners")
            .build();

        let scanner_stats_refresh_skips_total = meter
            .u64_counter("scanner_stats_refresh_skips_total")
            .with_description("Total number of stats refreshes skipped due to unchanged dataset")
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
            worker_ready,
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
            inference_embed_requests_total,
            inference_embed_duration,
            inference_embed_items_total,
            inference_embed_per_item_duration,
            inference_rerank_requests_total,
            inference_rerank_duration,
            inference_rerank_documents_total,
            inference_llm_requests_total,
            inference_llm_duration,
            inference_llm_tokens_generated,
            inference_llm_tokens_per_second,
            document_upload_per_item_duration,
            document_extraction_duration,
            document_chunking_per_item_duration,
            embedding_per_chunk_duration,
            llm_response_duration,
            chat_request_duration,
            visualization_fetch_vectors_duration,
            visualization_umap_duration,
            visualization_hdbscan_duration,
            visualization_plot_duration,
            storage_upload_duration,
            storage_download_duration,
            storage_delete_duration,
            storage_list_duration,
            gpu_memory_used_bytes,
            gpu_memory_total_bytes,
            gpu_utilization_percent,
            gpu_memory_utilization_percent,
            embedding_session_resets_total,
            embedding_session_request_count,
            embedding_session_age_seconds,
            dlq_messages_total,
            scanner_triggers_published_total,
            scanner_triggers_processed_total,
            scanner_items_discovered_total,
            scanner_scan_duration,
            scanner_backpressure_skips_total,
            scanner_failed_batch_recoveries_total,
            scanner_orphaned_batch_cleanups_total,
            scanner_pending_batch_recoveries_total,
            scanner_circuit_breaker_trips_total,
            scanner_batches_created_total,
            scanner_stats_refresh_skips_total,
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

pub fn set_worker_ready(worker: &str, ready: bool) {
    let metrics = get_metrics();
    let value = if ready { 1.0 } else { 0.0 };

    metrics
        .worker_ready
        .record(value, &[KeyValue::new("worker", worker.to_string())]);
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

/// Record a message sent to the Dead Letter Queue
pub fn record_dlq_message(transform_type: &str, reason: &str) {
    let metrics = get_metrics();
    metrics.dlq_messages_total.add(
        1,
        &[
            KeyValue::new("transform_type", transform_type.to_string()),
            KeyValue::new("reason", reason.to_string()),
        ],
    );
}

/// Record a scanner trigger being published
pub fn record_scanner_trigger_published(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_triggers_published_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Record a scanner trigger being processed
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

/// Record items discovered by a scanner
pub fn record_scanner_items_discovered(scanner_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_items_discovered_total.add(
        count,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Record the duration of a scanner scan
pub fn record_scanner_scan_duration(scanner_type: &str, duration_secs: f64) {
    let metrics = get_metrics();
    metrics.scanner_scan_duration.record(
        duration_secs,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Record a scan skipped due to backpressure (#10)
pub fn record_scanner_backpressure_skip(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_backpressure_skips_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Record a failed batch recovery (#10)
pub fn record_scanner_failed_batch_recovery(scanner_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_failed_batch_recoveries_total.add(
        count,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Record orphaned batch cleanups (#10)
pub fn record_scanner_orphaned_batch_cleanup(count: u64) {
    let metrics = get_metrics();
    metrics
        .scanner_orphaned_batch_cleanups_total
        .add(count, &[]);
}

/// Record pending batch recoveries (#10)
pub fn record_scanner_pending_batch_recovery(batch_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_pending_batch_recoveries_total.add(
        count,
        &[KeyValue::new("batch_type", batch_type.to_string())],
    );
}

/// Record a circuit breaker trip in the scanner (#10)
pub fn record_scanner_circuit_breaker_trip(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_circuit_breaker_trips_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Record batches created by a scanner (#10)
pub fn record_scanner_batches_created(scanner_type: &str, count: u64) {
    let metrics = get_metrics();
    metrics.scanner_batches_created_total.add(
        count,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Record a stats refresh skip due to unchanged dataset (#10)
pub fn record_scanner_stats_refresh_skip(scanner_type: &str) {
    let metrics = get_metrics();
    metrics.scanner_stats_refresh_skips_total.add(
        1,
        &[KeyValue::new("scanner_type", scanner_type.to_string())],
    );
}

/// Generic counter increment function for custom metrics
/// Use this for ad-hoc metric tracking when specific functions don't exist
pub fn increment_counter(_name: &str, labels: &[(&str, &str)]) {
    let metrics = get_metrics();
    let attributes: Vec<KeyValue> = labels
        .iter()
        .map(|(k, v)| KeyValue::new(k.to_string(), v.to_string()))
        .collect();

    // Try to find existing counter or create a new one
    // Note: This is a simplified implementation - for production use,
    // consider pre-registering counters in the Metrics struct
    metrics.http_requests_total.add(1, &attributes);
}

/// Generic histogram recording function for custom metrics
/// Use this for ad-hoc metric tracking when specific functions don't exist
pub fn record_histogram(_name: &str, value: f64, labels: &[(&str, &str)]) {
    let metrics = get_metrics();
    let attributes: Vec<KeyValue> = labels
        .iter()
        .map(|(k, v)| KeyValue::new(k.to_string(), v.to_string()))
        .collect();

    // Try to find existing histogram or use a default one
    // Note: This is a simplified implementation - for production use,
    // consider pre-registering histograms in the Metrics struct
    metrics.http_request_duration.record(value, &attributes);
}
/// Record embedding request metrics
pub fn record_embed_request(model: &str, item_count: u64, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.inference_embed_requests_total.add(
        1,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.inference_embed_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if success {
        metrics
            .inference_embed_items_total
            .add(item_count, &[KeyValue::new("model", model.to_string())]);

        // Calculate per-item duration
        if item_count > 0 {
            let per_item_duration = duration_secs / item_count as f64;
            metrics.inference_embed_per_item_duration.record(
                per_item_duration,
                &[KeyValue::new("model", model.to_string())],
            );
        }
    }
}

/// Record reranking request metrics
pub fn record_rerank_request(model: &str, document_count: u64, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.inference_rerank_requests_total.add(
        1,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.inference_rerank_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if success {
        metrics
            .inference_rerank_documents_total
            .add(document_count, &[KeyValue::new("model", model.to_string())]);
    }
}

/// Record LLM generation request metrics
pub fn record_llm_request(model: &str, tokens_generated: u64, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.inference_llm_requests_total.add(
        1,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    metrics.inference_llm_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );

    if success {
        metrics.inference_llm_tokens_generated.add(
            tokens_generated,
            &[KeyValue::new("model", model.to_string())],
        );

        // Calculate tokens per second (throughput)
        if duration_secs > 0.0 {
            let tokens_per_sec = tokens_generated as f64 / duration_secs;
            metrics
                .inference_llm_tokens_per_second
                .record(tokens_per_sec, &[KeyValue::new("model", model.to_string())]);
        }
    }
}

/// Record document upload timing per item
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

/// Record document extraction timing
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

/// Record document chunking timing per item
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

/// Record embedding generation timing per chunk
pub fn record_embedding_per_chunk(model: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.embedding_per_chunk_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

/// Record LLM response generation timing
pub fn record_llm_response(model: &str, duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.llm_response_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
        ],
    );
}

/// Record chat request timing
pub fn record_chat_request(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.chat_request_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

/// Record visualization vector fetch timing
pub fn record_visualization_fetch_vectors(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_fetch_vectors_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

/// Record visualization UMAP timing
pub fn record_visualization_umap(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_umap_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

/// Record visualization HDBSCAN timing
pub fn record_visualization_hdbscan(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_hdbscan_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

/// Record visualization plot generation timing
pub fn record_visualization_plot(duration_secs: f64, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    metrics.visualization_plot_duration.record(
        duration_secs,
        &[KeyValue::new("status", status.to_string())],
    );
}

/// Record S3 upload operation timing
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

/// Record S3 download operation timing
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

/// Record S3 delete operation timing
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

/// Record S3 list operation timing
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

/// Initialize observability for API services (actix-web based services)
/// Returns PrometheusMetrics that can be attached to actix-web App
pub fn init_observability_api(
    service_prefix: &str,
    endpoints_to_exclude: &[(&str, Option<&str>)],
    service_name: &str,
    otlp_endpoint: &str,
    log_format: &str,
) -> Result<PrometheusMetrics> {
    let use_json = log_format.to_lowercase() == "json";

    let resource = Resource::builder()
        .with_service_name(service_name.to_string())
        .build();

    let grpc_endpoint = if otlp_endpoint.ends_with("/v1/traces") {
        otlp_endpoint.trim_end_matches("/v1/traces").to_string()
    } else {
        otlp_endpoint.to_string()
    };

    // Build span exporter with proper timeout configuration
    let trace_exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(grpc_endpoint.clone())
        .with_timeout(Duration::from_secs(10))
        .build()?;

    // Configure batch exporter to not exceed max message size
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(trace_exporter)
        .with_resource(resource.clone())
        .with_id_generator(RandomIdGenerator::default())
        .with_sampler(Sampler::AlwaysOn)
        .build();
    global::set_tracer_provider(tracer_provider);
    tracing::info!("OpenTelemetry tracer initialized successfully");

    let log_exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(grpc_endpoint)
        .with_timeout(Duration::from_secs(10))
        .build()?;

    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(resource.clone())
        .build();

    let mut prometheus_builder = PrometheusMetricsBuilder::new(service_prefix).endpoint("/metrics");

    // Add exclusions
    for (path, regex_pattern) in endpoints_to_exclude {
        if let Some(regex) = regex_pattern {
            prometheus_builder = prometheus_builder.exclude_regex(*regex);
        } else {
            prometheus_builder = prometheus_builder.exclude(*path);
        }
    }

    let prometheus = prometheus_builder
        .build()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Share the Prometheus registry between actix-web-prom and OpenTelemetry
    // This ensures all metrics (HTTP, GPU, embedding sessions) are exposed via /metrics
    let shared_registry = prometheus.registry.clone();

    let prometheus_exporter = opentelemetry_prometheus::exporter()
        .with_registry(shared_registry)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build prometheus exporter: {}", e))?;

    let meter_provider = SdkMeterProvider::builder()
        .with_reader(prometheus_exporter)
        .with_resource(resource.clone())
        .build();

    global::set_meter_provider(meter_provider);
    global::set_text_map_propagator(TraceContextPropagator::new());

    init_metrics_otel().map_err(|e| anyhow::anyhow!("Failed to initialize core metrics: {}", e))?;

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("failed to initialize tracing filter layer");

    // Use JSON format for structured logging in production, human-readable for development
    let format_layer = if use_json {
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(false)
            .with_target(true)
            .with_file(true)
            .flatten_event(true)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .with_ansi(true)
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .boxed()
    };

    let tracer = global::tracer_provider().tracer(service_name.to_string());
    let otel_trace_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);

    Registry::default()
        .with(env_filter)
        .with(format_layer)
        .with(otel_trace_layer)
        .with(otel_log_layer)
        .try_init()?;

    Ok(prometheus)
}

/// Record embedding generation for a batch - more efficient than per-chunk recording
/// Records total batch duration and chunk count to reduce observability overhead
pub fn record_embedding_batch(model: &str, duration_secs: f64, chunk_count: usize, success: bool) {
    let metrics = get_metrics();
    let status = if success { "success" } else { "error" };

    // Record total batch duration with chunk count metadata
    metrics.embedding_per_chunk_duration.record(
        duration_secs,
        &[
            KeyValue::new("model", model.to_string()),
            KeyValue::new("status", status.to_string()),
            KeyValue::new("batch", "true"),
            KeyValue::new("chunk_count", chunk_count.to_string()),
        ],
    );
}

// ============================================================================
// GPU Metrics Recording Functions
// ============================================================================

/// Record GPU memory and utilization metrics
pub fn record_gpu_metrics(
    device_index: u32,
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    gpu_utilization: u32,
    memory_utilization: u32,
) {
    let metrics = get_metrics();
    let device_label = device_index.to_string();

    metrics.gpu_memory_used_bytes.record(
        memory_used_bytes as f64,
        &[KeyValue::new("device", device_label.clone())],
    );

    metrics.gpu_memory_total_bytes.record(
        memory_total_bytes as f64,
        &[KeyValue::new("device", device_label.clone())],
    );

    metrics.gpu_utilization_percent.record(
        gpu_utilization as f64,
        &[KeyValue::new("device", device_label.clone())],
    );

    metrics.gpu_memory_utilization_percent.record(
        memory_utilization as f64,
        &[KeyValue::new("device", device_label)],
    );
}

/// Record embedding session lifecycle metrics
pub fn record_embedding_session_metrics(model_id: &str, request_count: u64, age_seconds: f64) {
    let metrics = get_metrics();

    metrics.embedding_session_request_count.record(
        request_count as f64,
        &[KeyValue::new("model", model_id.to_string())],
    );

    metrics
        .embedding_session_age_seconds
        .record(age_seconds, &[KeyValue::new("model", model_id.to_string())]);
}

/// Record embedding session reset event
pub fn record_embedding_session_reset(model_id: &str, reason: &str) {
    let metrics = get_metrics();

    metrics.embedding_session_resets_total.add(
        1,
        &[
            KeyValue::new("model", model_id.to_string()),
            KeyValue::new("reason", reason.to_string()),
        ],
    );
}

/// Initialize embedding session reset counter for a model so it appears in metrics
/// Call this at model load time to ensure the metric exists with zero value
pub fn init_embedding_session_reset_metric(model_id: &str) {
    let metrics = get_metrics();

    // Add 0 to initialize the counter so it appears in Prometheus
    metrics.embedding_session_resets_total.add(
        0,
        &[
            KeyValue::new("model", model_id.to_string()),
            KeyValue::new("reason", "none".to_string()),
        ],
    );
}

/// GPU metrics collector using NVML
/// Polls GPU memory and utilization at regular intervals
pub mod gpu_monitor {
    use nvml_wrapper::Nvml;
    use std::sync::OnceLock;
    use std::time::Duration;
    use tracing::{debug, info, warn};

    static NVML: OnceLock<Option<Nvml>> = OnceLock::new();

    /// Initialize NVML for GPU monitoring
    /// Returns true if NVML is available, false otherwise
    pub fn init() -> bool {
        let nvml = NVML.get_or_init(|| match Nvml::init() {
            Ok(nvml) => {
                info!("NVML initialized successfully for GPU monitoring");
                Some(nvml)
            }
            Err(e) => {
                warn!(
                    "NVML initialization failed (no GPU or drivers not installed): {}",
                    e
                );
                None
            }
        });
        nvml.is_some()
    }

    /// Get current GPU memory usage for a device
    /// Returns (used_bytes, total_bytes) or None if unavailable
    pub fn get_memory_info(device_index: u32) -> Option<(u64, u64)> {
        let nvml = NVML.get()?.as_ref()?;
        let device = nvml.device_by_index(device_index).ok()?;
        let memory = device.memory_info().ok()?;
        Some((memory.used, memory.total))
    }

    /// Get GPU utilization percentage
    /// Returns (gpu_util, memory_util) or None if unavailable
    pub fn get_utilization(device_index: u32) -> Option<(u32, u32)> {
        let nvml = NVML.get()?.as_ref()?;
        let device = nvml.device_by_index(device_index).ok()?;
        let utilization = device.utilization_rates().ok()?;
        Some((utilization.gpu, utilization.memory))
    }

    /// Get the number of GPU devices
    pub fn device_count() -> u32 {
        NVML.get()
            .and_then(|nvml| nvml.as_ref())
            .and_then(|nvml| nvml.device_count().ok())
            .unwrap_or(0)
    }

    /// Check if GPU memory utilization exceeds threshold
    /// Returns true if any device exceeds the threshold percentage
    pub fn is_memory_pressure_high(threshold_percent: f64) -> bool {
        let count = device_count();
        for i in 0..count {
            if let Some((used, total)) = get_memory_info(i) {
                let utilization = (used as f64 / total as f64) * 100.0;
                if utilization > threshold_percent {
                    return true;
                }
            }
        }
        false
    }

    /// Get memory utilization percentage for a device
    pub fn get_memory_utilization_percent(device_index: u32) -> Option<f64> {
        let (used, total) = get_memory_info(device_index)?;
        Some((used as f64 / total as f64) * 100.0)
    }

    /// Collect and record all GPU metrics
    /// Call this periodically from a background task
    pub fn collect_metrics() {
        let count = device_count();
        for i in 0..count {
            if let (Some((used, total)), Some((gpu_util, mem_util))) =
                (get_memory_info(i), get_utilization(i))
            {
                super::record_gpu_metrics(i, used, total, gpu_util, mem_util);
                debug!(
                    device = i,
                    used_mb = used / 1024 / 1024,
                    total_mb = total / 1024 / 1024,
                    gpu_util = gpu_util,
                    mem_util = mem_util,
                    "GPU metrics collected"
                );
            }
        }
    }

    /// Spawn a background task to collect GPU metrics at regular intervals
    /// Returns a JoinHandle that can be used to cancel the task
    pub fn spawn_metrics_collector(interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if !init() {
                warn!("GPU monitoring disabled - NVML not available");
                return;
            }

            info!(
                interval_secs = interval.as_secs(),
                device_count = device_count(),
                "Starting GPU metrics collector"
            );

            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                collect_metrics();
            }
        })
    }
}
