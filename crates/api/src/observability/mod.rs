use actix_web_prom::{PrometheusMetrics, PrometheusMetricsBuilder};
use anyhow::{Result, anyhow};
use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    logs::SdkLoggerProvider,
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use std::{env, time::Duration};
use tracing_subscriber::{
    EnvFilter, Layer, Registry, layer::SubscriberExt, util::SubscriberInitExt,
};

pub(crate) fn init_observability() -> Result<PrometheusMetrics> {
    let service_name = env::var("SERVICE_NAME").unwrap_or_else(|_| "semantic-explorer".to_string());

    let resource = Resource::builder()
        .with_service_name(service_name.clone())
        .build();
    let otlp_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    let grpc_endpoint = if otlp_endpoint.ends_with("/v1/traces") {
        otlp_endpoint.trim_end_matches("/v1/traces").to_string()
    } else {
        otlp_endpoint.clone()
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

    let prometheus = PrometheusMetricsBuilder::new("api")
        .endpoint("/metrics")
        .exclude("/")
        .exclude("/health")
        .exclude("/metrics")
        .exclude("/swagger-ui")
        .exclude_regex("/swagger-ui/.*")
        .exclude("/ui")
        .exclude_regex("/ui/.*")
        .exclude_regex("/.well-known/.*")
        .build()
        .map_err(|e| anyhow!(e.to_string()))?;

    let prometheus_exporter = opentelemetry_prometheus::exporter().build()?;

    let meter_provider = SdkMeterProvider::builder()
        .with_reader(prometheus_exporter)
        .with_resource(resource.clone())
        .build();

    global::set_meter_provider(meter_provider);
    global::set_text_map_propagator(TraceContextPropagator::new());

    semantic_explorer_core::observability::init_metrics_otel()
        .map_err(|e| anyhow!("Failed to initialize core metrics: {}", e))?;

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("failed to initialize tracing filter layer");

    // Use JSON format for structured logging in production, human-readable for development
    let use_json = env::var("LOG_FORMAT")
        .unwrap_or_else(|_| "json".to_string())
        .to_lowercase()
        == "json";

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

    let tracer = global::tracer_provider().tracer(service_name.clone());
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
