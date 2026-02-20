mod database;
mod gpu;
mod inference;
mod nats;
mod pipeline;
mod scanner;
mod search;
mod storage;
mod valkey;
mod worker;

pub use database::*;
pub use gpu::{gpu_monitor, record_gpu_metrics};
pub use inference::*;
pub use nats::*;
pub use pipeline::*;
pub use scanner::*;
pub use search::*;
pub use storage::*;
pub use valkey::*;
pub use worker::*;

use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use anyhow::Result;
use opentelemetry::{
    global,
    metrics::{Counter, Gauge, Histogram, Meter},
    trace::TracerProvider,
};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    logs::SdkLoggerProvider,
    metrics::{Aggregation, Instrument, SdkMeterProvider, Stream},
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use std::{env, sync::OnceLock, time::Duration};
use tracing_subscriber::{
    EnvFilter, Layer, Registry, layer::SubscriberExt, util::SubscriberInitExt,
};

pub(crate) struct Metrics {
    pub database_connection_pool_size: Gauge<f64>,
    pub database_connection_pool_idle: Gauge<f64>,
    pub database_connection_pool_max: Gauge<f64>,
    pub storage_operations_total: Counter<u64>,
    pub storage_operation_duration: Histogram<f64>,
    pub storage_file_size_bytes: Histogram<f64>,
    pub worker_ready: Gauge<f64>,
    pub worker_jobs_total: Counter<u64>,
    pub worker_job_duration: Histogram<f64>,
    pub worker_job_chunks: Histogram<f64>,
    pub worker_job_file_size: Histogram<f64>,
    pub embedded_datasets_active: Gauge<f64>,
    pub nats_stream_messages: Gauge<f64>,
    pub nats_consumer_pending: Gauge<f64>,
    pub nats_consumer_ack_pending: Gauge<f64>,
    pub nats_stream_bytes: Gauge<f64>,
    pub search_request_total: Counter<u64>,
    pub search_request_duration: Histogram<f64>,
    pub search_embedder_call_duration: Histogram<f64>,
    pub search_qdrant_query_duration: Histogram<f64>,
    pub search_results_returned: Histogram<f64>,
    pub worker_job_failures_total: Counter<u64>,
    pub worker_job_retries_total: Counter<u64>,
    pub inference_embed_requests_total: Counter<u64>,
    pub inference_embed_duration: Histogram<f64>,
    pub inference_embed_items_total: Counter<u64>,
    pub inference_embed_per_item_duration: Histogram<f64>,
    pub inference_rerank_requests_total: Counter<u64>,
    pub inference_rerank_duration: Histogram<f64>,
    pub inference_rerank_documents_total: Counter<u64>,
    pub inference_llm_requests_total: Counter<u64>,
    pub inference_llm_duration: Histogram<f64>,
    pub inference_llm_tokens_generated: Counter<u64>,
    pub inference_llm_tokens_per_second: Histogram<f64>,
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
    pub gpu_memory_used_bytes: Gauge<f64>,
    pub gpu_memory_total_bytes: Gauge<f64>,
    pub gpu_utilization_percent: Gauge<f64>,
    pub gpu_memory_utilization_percent: Gauge<f64>,
    pub embedding_session_resets_total: Counter<u64>,
    pub embedding_session_request_count: Gauge<f64>,
    pub embedding_session_age_seconds: Gauge<f64>,
    pub dlq_messages_total: Counter<u64>,
    pub scanner_triggers_published_total: Counter<u64>,
    pub scanner_triggers_processed_total: Counter<u64>,
    pub scanner_items_discovered_total: Counter<u64>,
    pub scanner_scan_duration: Histogram<f64>,
    pub scanner_backpressure_skips_total: Counter<u64>,
    pub scanner_failed_batch_recoveries_total: Counter<u64>,
    pub scanner_orphaned_batch_cleanups_total: Counter<u64>,
    pub scanner_pending_batch_recoveries_total: Counter<u64>,
    pub scanner_circuit_breaker_trips_total: Counter<u64>,
    pub scanner_batches_created_total: Counter<u64>,
    pub scanner_stats_refresh_skips_total: Counter<u64>,
    pub valkey_cache_hits_total: Counter<u64>,
    pub valkey_cache_misses_total: Counter<u64>,
    pub valkey_cache_errors_total: Counter<u64>,
    pub valkey_operation_duration: Histogram<f64>,
    pub valkey_connected: Gauge<f64>,
    pub valkey_used_memory_bytes: Gauge<f64>,
    pub valkey_connected_clients: Gauge<f64>,
    pub valkey_keyspace_hits: Gauge<f64>,
    pub valkey_keyspace_misses: Gauge<f64>,
    pub bearer_l1_cache_hits_total: Counter<u64>,
    pub bearer_l1_cache_misses_total: Counter<u64>,
    pub bearer_l1_cache_entries: Gauge<f64>,
}

impl Metrics {
    fn new(meter: Meter) -> Self {
        let database_connection_pool_size = meter
            .f64_gauge("database_connection_pool_size")
            .with_description("Number of connections allocated in the pool")
            .build();

        let database_connection_pool_idle = meter
            .f64_gauge("database_connection_pool_idle")
            .with_description("Number of idle database connections")
            .build();

        let database_connection_pool_max = meter
            .f64_gauge("database_connection_pool_max")
            .with_description("Maximum configured database connections (DB_MAX_CONNECTIONS)")
            .build();

        let storage_operations_total = meter
            .u64_counter("storage_operations")
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
            .u64_counter("worker_jobs")
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

        let embedded_datasets_active = meter
            .f64_gauge("embedded_datasets_active")
            .with_description("Number of active embedded datasets")
            .build();

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

        let search_request_total = meter
            .u64_counter("search_request")
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

        let worker_job_failures_total = meter
            .u64_counter("worker_job_failures")
            .with_description("Total number of worker job failures")
            .build();

        let worker_job_retries_total = meter
            .u64_counter("worker_job_retries")
            .with_description("Total number of worker job retries")
            .build();

        let inference_embed_requests_total = meter
            .u64_counter("inference_embed_requests")
            .with_description("Total number of embedding requests")
            .build();

        let inference_embed_duration = meter
            .f64_histogram("inference_embed_duration_seconds")
            .with_description("Duration of embedding requests in seconds")
            .build();

        let inference_embed_items_total = meter
            .u64_counter("inference_embed_items")
            .with_description("Total number of items embedded")
            .build();

        let inference_embed_per_item_duration = meter
            .f64_histogram("inference_embed_per_item_duration_seconds")
            .with_description("Average duration per item in embedding requests")
            .build();

        let inference_rerank_requests_total = meter
            .u64_counter("inference_rerank_requests")
            .with_description("Total number of reranking requests")
            .build();

        let inference_rerank_duration = meter
            .f64_histogram("inference_rerank_duration_seconds")
            .with_description("Duration of reranking requests in seconds")
            .build();

        let inference_rerank_documents_total = meter
            .u64_counter("inference_rerank_documents")
            .with_description("Total number of documents reranked")
            .build();

        let inference_llm_requests_total = meter
            .u64_counter("inference_llm_requests")
            .with_description("Total number of LLM generation requests")
            .build();

        let inference_llm_duration = meter
            .f64_histogram("inference_llm_duration_seconds")
            .with_description("Duration of LLM generation requests in seconds")
            .build();

        let inference_llm_tokens_generated = meter
            .u64_counter("inference_llm_tokens_generated")
            .with_description("Total number of tokens generated by LLMs")
            .build();

        let inference_llm_tokens_per_second = meter
            .f64_histogram("inference_llm_tokens_per_second")
            .with_description("Tokens generated per second (throughput)")
            .build();

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

        let embedding_session_resets_total = meter
            .u64_counter("embedding_session_resets")
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
            .u64_counter("dlq_messages")
            .with_description("Total number of messages sent to Dead Letter Queue")
            .build();

        let scanner_triggers_published_total = meter
            .u64_counter("scanner_triggers_published")
            .with_description("Total number of scanner triggers published")
            .build();

        let scanner_triggers_processed_total = meter
            .u64_counter("scanner_triggers_processed")
            .with_description("Total number of scanner triggers processed")
            .build();

        let scanner_items_discovered_total = meter
            .u64_counter("scanner_items_discovered")
            .with_description("Total number of items discovered by scanners")
            .build();

        let scanner_scan_duration = meter
            .f64_histogram("scanner_scan_duration_seconds")
            .with_description("Duration of scanner scans in seconds")
            .build();

        let scanner_backpressure_skips_total = meter
            .u64_counter("scanner_backpressure_skips")
            .with_description("Total number of scans skipped due to backpressure")
            .build();

        let scanner_failed_batch_recoveries_total = meter
            .u64_counter("scanner_failed_batch_recoveries")
            .with_description("Total number of failed batches recovered by reconciliation")
            .build();

        let scanner_orphaned_batch_cleanups_total = meter
            .u64_counter("scanner_orphaned_batch_cleanups")
            .with_description("Total number of orphaned batches cleaned up")
            .build();

        let scanner_pending_batch_recoveries_total = meter
            .u64_counter("scanner_pending_batch_recoveries")
            .with_description("Total number of pending batches recovered")
            .build();

        let scanner_circuit_breaker_trips_total = meter
            .u64_counter("scanner_circuit_breaker_trips")
            .with_description("Total number of circuit breaker trips in scanner")
            .build();

        let scanner_batches_created_total = meter
            .u64_counter("scanner_batches_created")
            .with_description("Total number of batches created by scanners")
            .build();

        let scanner_stats_refresh_skips_total = meter
            .u64_counter("scanner_stats_refresh_skips")
            .with_description("Total number of stats refreshes skipped due to unchanged dataset")
            .build();

        let valkey_cache_hits_total = meter
            .u64_counter("valkey_cache_hits")
            .with_description("Total number of Valkey cache hits")
            .build();

        let valkey_cache_misses_total = meter
            .u64_counter("valkey_cache_misses")
            .with_description("Total number of Valkey cache misses")
            .build();

        let valkey_cache_errors_total = meter
            .u64_counter("valkey_cache_errors")
            .with_description("Total number of Valkey cache errors (connection failures, timeouts)")
            .build();

        let valkey_operation_duration = meter
            .f64_histogram("valkey_operation_duration_seconds")
            .with_description("Duration of Valkey cache operations in seconds")
            .build();

        let valkey_connected = meter
            .f64_gauge("valkey_connected")
            .with_description("Valkey connection status (1 = connected, 0 = disconnected)")
            .build();

        let valkey_used_memory_bytes = meter
            .f64_gauge("valkey_used_memory_bytes")
            .with_description("Valkey server used memory in bytes")
            .build();

        let valkey_connected_clients = meter
            .f64_gauge("valkey_connected_clients")
            .with_description("Number of clients connected to Valkey")
            .build();

        let valkey_keyspace_hits = meter
            .f64_gauge("valkey_keyspace_hits")
            .with_description("Valkey server-side keyspace hits (cumulative)")
            .build();

        let valkey_keyspace_misses = meter
            .f64_gauge("valkey_keyspace_misses")
            .with_description("Valkey server-side keyspace misses (cumulative)")
            .build();

        let bearer_l1_cache_hits_total = meter
            .u64_counter("bearer_l1_cache_hits")
            .with_description("Total number of bearer token L1 in-memory cache hits")
            .build();

        let bearer_l1_cache_misses_total = meter
            .u64_counter("bearer_l1_cache_misses")
            .with_description("Total number of bearer token L1 in-memory cache misses")
            .build();

        let bearer_l1_cache_entries = meter
            .f64_gauge("bearer_l1_cache_entries")
            .with_description("Current number of entries in the bearer token L1 in-memory cache")
            .build();

        Self {
            database_connection_pool_size,
            database_connection_pool_idle,
            database_connection_pool_max,
            storage_operations_total,
            storage_operation_duration,
            storage_file_size_bytes,
            worker_ready,
            worker_jobs_total,
            worker_job_duration,
            worker_job_chunks,
            worker_job_file_size,
            embedded_datasets_active,
            nats_stream_messages,
            nats_consumer_pending,
            nats_consumer_ack_pending,
            nats_stream_bytes,
            search_request_total,
            search_request_duration,
            search_embedder_call_duration,
            search_qdrant_query_duration,
            search_results_returned,
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
            valkey_cache_hits_total,
            valkey_cache_misses_total,
            valkey_cache_errors_total,
            valkey_operation_duration,
            valkey_connected,
            valkey_used_memory_bytes,
            valkey_connected_clients,
            valkey_keyspace_hits,
            valkey_keyspace_misses,
            bearer_l1_cache_hits_total,
            bearer_l1_cache_misses_total,
            bearer_l1_cache_entries,
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

fn get_metrics() -> &'static Metrics {
    METRICS.get().expect("Metrics not initialized")
}

pub fn init_observability_api(
    service_prefix: &str,
    endpoints_to_exclude: &[(&str, Option<&str>)],
    service_name: &str,
    otlp_endpoint: &str,
    log_format: &str,
) -> Result<PrometheusMetrics> {
    let use_json = log_format.to_lowercase() == "json";

    let otel_enabled = env::var("OTEL_ENABLED")
        .unwrap_or_else(|_| "true".to_string())
        .to_lowercase()
        != "false";

    let otel_logs_enabled = otel_enabled
        && env::var("OTEL_LOGS_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            != "false";

    let sample_ratio: f64 = env::var("OTEL_SAMPLE_RATIO")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    let resource = Resource::builder()
        .with_service_name(service_name.to_string())
        .build();

    let grpc_endpoint = if otlp_endpoint.ends_with("/v1/traces") {
        otlp_endpoint.trim_end_matches("/v1/traces").to_string()
    } else {
        otlp_endpoint.to_string()
    };

    let trace_batch_config = opentelemetry_sdk::trace::BatchConfigBuilder::default()
        .with_max_queue_size(2048)
        .with_max_export_batch_size(512)
        .with_scheduled_delay(Duration::from_secs(5))
        .build();

    if otel_enabled {
        let trace_exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(grpc_endpoint.clone())
            .with_timeout(Duration::from_secs(5))
            .build()?;

        let trace_processor = opentelemetry_sdk::trace::BatchSpanProcessor::builder(trace_exporter)
            .with_batch_config(trace_batch_config)
            .build();

        let sampler = if (sample_ratio - 1.0).abs() < f64::EPSILON {
            Sampler::AlwaysOn
        } else if sample_ratio <= 0.0 {
            Sampler::AlwaysOff
        } else {
            Sampler::TraceIdRatioBased(sample_ratio)
        };

        let tracer_provider = SdkTracerProvider::builder()
            .with_span_processor(trace_processor)
            .with_resource(resource.clone())
            .with_id_generator(RandomIdGenerator::default())
            .with_sampler(sampler)
            .build();
        global::set_tracer_provider(tracer_provider);
        tracing::info!(
            "OpenTelemetry tracer initialized (sample_ratio={}, logs={})",
            sample_ratio,
            otel_logs_enabled
        );
    } else {
        let tracer_provider = SdkTracerProvider::builder()
            .with_resource(resource.clone())
            .with_sampler(Sampler::AlwaysOff)
            .build();
        global::set_tracer_provider(tracer_provider);
        tracing::info!("OpenTelemetry OTLP export disabled (OTEL_ENABLED=false)");
    }

    let logger_provider = if otel_logs_enabled {
        let log_batch_config = opentelemetry_sdk::logs::BatchConfigBuilder::default()
            .with_max_queue_size(2048)
            .with_scheduled_delay(Duration::from_secs(5))
            .build();

        let log_exporter = LogExporter::builder()
            .with_tonic()
            .with_endpoint(grpc_endpoint)
            .with_timeout(Duration::from_secs(5))
            .build()?;

        let log_processor = opentelemetry_sdk::logs::BatchLogProcessor::builder(log_exporter)
            .with_batch_config(log_batch_config)
            .build();

        Some(
            SdkLoggerProvider::builder()
                .with_log_processor(log_processor)
                .with_resource(resource.clone())
                .build(),
        )
    } else {
        None
    };

    let mut prometheus_builder = PrometheusMetricsBuilder::new(service_prefix).endpoint("/metrics");

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

    let shared_registry = prometheus.registry.clone();

    let prometheus_exporter = opentelemetry_prometheus::exporter()
        .with_registry(shared_registry)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build prometheus exporter: {}", e))?;

    let duration_boundaries: Vec<f64> = vec![
        0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
        60.0, 120.0, 300.0,
    ];

    let duration_view = {
        let boundaries = duration_boundaries.clone();
        move |inst: &Instrument| -> Option<Stream> {
            if inst.name().ends_with("_seconds") {
                Stream::builder()
                    .with_aggregation(Aggregation::ExplicitBucketHistogram {
                        boundaries: boundaries.clone(),
                        record_min_max: true,
                    })
                    .build()
                    .ok()
            } else {
                None
            }
        }
    };

    let meter_provider = SdkMeterProvider::builder()
        .with_reader(prometheus_exporter)
        .with_resource(resource.clone())
        .with_view(duration_view)
        .build();

    global::set_meter_provider(meter_provider);
    global::set_text_map_propagator(TraceContextPropagator::new());

    init_metrics_otel().map_err(|e| anyhow::anyhow!("Failed to initialize core metrics: {}", e))?;

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("failed to initialize tracing filter layer");

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

    if otel_enabled {
        let tracer = global::tracer_provider().tracer(service_name.to_string());
        let otel_trace_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        if let Some(ref lp) = logger_provider {
            let otel_log_layer = OpenTelemetryTracingBridge::new(lp);
            Registry::default()
                .with(env_filter)
                .with(format_layer)
                .with(otel_trace_layer)
                .with(otel_log_layer)
                .try_init()?;
        } else {
            Registry::default()
                .with(env_filter)
                .with(format_layer)
                .with(otel_trace_layer)
                .try_init()?;
        }
    } else {
        Registry::default()
            .with(env_filter)
            .with(format_layer)
            .try_init()?;
    }

    Ok(prometheus)
}
