//! Observability setup for the LLM inference API.
//!
//! Configures OpenTelemetry tracing, logging, and Prometheus metrics.

use actix_web_prom::PrometheusMetrics;
use anyhow::Result;

use crate::config::{LogFormat, ObservabilityConfig};

/// Initialize observability (tracing, logging, metrics)
pub fn init_observability(config: &ObservabilityConfig) -> Result<PrometheusMetrics> {
    let log_format = match config.log_format {
        LogFormat::Json => "json",
        LogFormat::Pretty => "pretty",
    };

    let endpoints_to_exclude = [
        ("/health/.*", Some("/health/.*")),
        ("/metrics", None),
        ("/swagger-ui", None),
        ("/swagger-ui/.*", Some("/swagger-ui/.*")),
    ];

    semantic_explorer_core::observability::init_observability_api(
        "llm-inference",
        &endpoints_to_exclude,
        &config.service_name,
        &config.otlp_endpoint,
        log_format,
    )
}
