"""
OpenTelemetry instrumentation for Visualization Worker

Provides metrics, tracing, and logging integration with the semantic-explorer
observability stack. Exports Prometheus metrics on port 9090.
"""

import json
import logging
import os
import sys
from datetime import datetime, timezone
from typing import Optional, Dict, Any

from prometheus_client import Counter, Gauge, Histogram, CollectorRegistry, start_http_server
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor, SimpleSpanProcessor
from opentelemetry.exporter.jaeger.thrift import JaegerExporter
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource


class Metrics:
    """Prometheus metrics for the visualization worker."""

    def __init__(self, registry: Optional[CollectorRegistry] = None):
        """Initialize metrics."""
        self.registry = registry

        # Job execution metrics - matching Rust worker structure
        self.visualization_jobs_total = Counter(
            "visualization_transform_jobs_total",
            "Total number of visualization transform jobs processed",
            ["status"],
            registry=registry,
        )

        self.visualization_job_duration = Histogram(
            "visualization_transform_duration_seconds",
            "Duration of visualization transform jobs in seconds",
            buckets=(0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, float("inf")),
            registry=registry,
        )

        self.visualization_points_created = Counter(
            "visualization_transform_points_created",
            "Total number of visualization points created",
            registry=registry,
        )

        self.visualization_clusters_created = Counter(
            "visualization_transform_clusters_created",
            "Total number of clusters created by visualization transforms",
            registry=registry,
        )

        # Processing stage metrics
        self.visualization_processing_duration = Histogram(
            "visualization_processing_duration_seconds",
            "Duration of the actual visualization processing (excluding I/O)",
            buckets=(0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, float("inf")),
            registry=registry,
        )

        self.visualization_s3_upload_duration = Histogram(
            "visualization_s3_upload_duration_seconds",
            "Duration of S3 uploads for visualization results",
            buckets=(0.1, 0.5, 1.0, 2.5, 5.0, 10.0, float("inf")),
            registry=registry,
        )

        # Job failure and retry tracking
        self.visualization_job_failures_total = Counter(
            "visualization_job_failures_total",
            "Total number of visualization job failures",
            ["error_type"],
            registry=registry,
        )

        self.visualization_job_retries_total = Counter(
            "visualization_job_retries_total",
            "Total number of visualization job retries",
            registry=registry,
        )

        # NATS message metrics
        self.nats_messages_received_total = Counter(
            "nats_messages_received_total",
            "Total number of NATS messages received",
            registry=registry,
        )

        self.nats_messages_acked_total = Counter(
            "nats_messages_acked_total",
            "Total number of NATS messages acknowledged",
            registry=registry,
        )

        self.nats_messages_nacked_total = Counter(
            "nats_messages_nacked_total",
            "Total number of NATS messages nacked (negative acknowledged)",
            registry=registry,
        )

        # Worker state metrics
        self.active_jobs_gauge = Gauge(
            "visualization_active_jobs",
            "Number of visualization jobs currently being processed",
            registry=registry,
        )

        self.worker_ready = Gauge(
            "visualization_worker_ready",
            "1 if worker is ready, 0 otherwise",
            registry=registry,
        )

        # Component initialization times
        self.s3_init_duration = Histogram(
            "visualization_s3_init_duration_seconds",
            "Duration of S3 storage initialization",
            buckets=(0.1, 0.5, 1.0, 5.0, 10.0, float("inf")),
            registry=registry,
        )

        self.llm_init_duration = Histogram(
            "visualization_llm_init_duration_seconds",
            "Duration of LLM provider initialization",
            buckets=(0.1, 0.5, 1.0, 5.0, 10.0, float("inf")),
            registry=registry,
        )


def setup_observability(worker_id: str, service_name: str = "worker-visualizations") -> Metrics:
    """
    Initialize OpenTelemetry and Prometheus for the worker.

    Args:
        worker_id: Unique identifier for this worker instance
        service_name: Service name for tracing/metrics

    Returns:
        Metrics: Initialized metrics instance
    """
    # Configuration from environment
    otlp_endpoint = os.getenv("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317")
    prometheus_port = int(os.getenv("PROMETHEUS_METRICS_PORT", "9090"))
    log_level = os.getenv("LOG_LEVEL", "INFO")
    log_format = os.getenv("LOG_FORMAT", "json").lower()

    # Resource for identifying this worker
    resource = Resource.create({
        "service.name": service_name,
        "service.version": "1.0.0",
        "service.instance.id": worker_id,
    })

    # Initialize Prometheus metrics with default registry
    metrics_obj = Metrics()

    # Start Prometheus HTTP server for metrics scraping
    try:
        start_http_server(prometheus_port)
        logging.info(f"Prometheus metrics server started on port {prometheus_port}")
    except OSError as e:
        logging.error(f"Failed to start Prometheus metrics server on port {prometheus_port}: {e}")

    # Setup Tracing
    try:
        trace_provider = TracerProvider(resource=resource)

        # Add Jaeger exporter for traces
        jaeger_exporter = JaegerExporter(
            agent_host_name="localhost",
            agent_port=6831,
        )
        trace_provider.add_span_processor(SimpleSpanProcessor(jaeger_exporter))

        # Also add OTLP exporter for traces
        otlp_span_exporter = OTLPSpanExporter(endpoint=otlp_endpoint)
        trace_provider.add_span_processor(BatchSpanProcessor(otlp_span_exporter))

        trace.set_tracer_provider(trace_provider)
        logging.debug("Tracing configured with Jaeger and OTLP exporters")
    except Exception as e:  # noqa: BLE001
        logging.warning(f"Failed to initialize tracing: {e}")

    # Configure structured logging
    configure_structured_logging(worker_id, log_format, log_level)

    return metrics_obj


def configure_structured_logging(worker_id: str, log_format: str = "json", log_level: str = "INFO"):
    """Configure structured logging."""
    root_logger = logging.getLogger()
    root_logger.setLevel(getattr(logging, log_level))

    # Remove existing handlers
    for handler in root_logger.handlers[:]:
        root_logger.removeHandler(handler)

    # Create new handler
    handler = logging.StreamHandler(sys.stdout)

    if log_format == "json":
        class JSONFormatter(logging.Formatter):
            def format(self, record):
                log_obj: Dict[str, Any] = {
                    "timestamp": datetime.fromtimestamp(record.created, tz=timezone.utc).isoformat(),
                    "level": record.levelname,
                    "logger": record.name,
                    "message": record.getMessage(),
                    "worker_id": worker_id,
                }
                if record.exc_info:
                    log_obj["exception"] = self.formatException(record.exc_info)
                return json.dumps(log_obj)

        handler.setFormatter(JSONFormatter())
    else:
        handler.setFormatter(
            logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
        )

    root_logger.addHandler(handler)


# Global metrics instance
_metrics: Optional[Metrics] = None


def get_metrics() -> Metrics:
    """Get the global metrics instance."""
    global _metrics
    if _metrics is None:
        raise RuntimeError("Metrics not initialized. Call setup_observability() first.")
    return _metrics


def init_metrics(worker_id: str) -> Metrics:
    """Initialize global metrics instance."""
    global _metrics
    _metrics = setup_observability(worker_id)
    return _metrics
