use actix_web_prom::PrometheusMetrics;
use anyhow::Result;
use std::env;

pub(crate) fn init_observability() -> Result<PrometheusMetrics> {
    let service_name = env::var("SERVICE_NAME").unwrap_or_else(|_| "semantic-explorer".to_string());
    let otlp_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());
    let log_format = env::var("LOG_FORMAT").unwrap_or_else(|_| "json".to_string());

    let endpoints_to_exclude = [
        ("/", None),
        ("/health/.*", Some("/health/.*")),
        ("/metrics", None),
        ("/swagger-ui", None),
        ("/swagger-ui/.*", Some("/swagger-ui/.*")),
        ("/ui", None),
        ("/ui/.*", Some("/ui/.*")),
        ("/.well-known/.*", Some("/.well-known/.*")),
    ];

    semantic_explorer_core::observability::init_observability_api(
        "api",
        &endpoints_to_exclude,
        &service_name,
        &otlp_endpoint,
        &log_format,
    )
}
