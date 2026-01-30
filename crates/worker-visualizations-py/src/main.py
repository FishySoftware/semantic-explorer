#!/usr/bin/env python3
"""
Visualization Worker - Main Entry Point

Subscribes to NATS queue, processes visualization transform jobs,
and publishes results back to the result topic.
"""

import asyncio
import json
import logging
import os
import signal
import sys
import time
import uuid
import nats
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional
from aiohttp import web
from nats.aio.msg import Msg
from nats.aio.client import Client as NATSConnection
from nats.js.api import ConsumerConfig
from pydantic import ValidationError
from dotenv import load_dotenv

# OpenTelemetry trace context propagation
from opentelemetry import trace
from opentelemetry.propagate import extract
from opentelemetry.trace import SpanKind

# Load environment variables from .env file
# Look for .env in the parent directory (crates/worker-visualizations-py/)
env_path = Path(__file__).parent.parent / ".env"
if env_path.exists():
    load_dotenv(dotenv_path=env_path)
    print(f"Loaded environment from {env_path}")
else:
    # Try loading from current directory as fallback
    load_dotenv()

try:
    # Try relative imports (for package execution)
    from .font_initializer import init_fonts_for_offline_mode
    from .processor import process_visualization_job
    from .models import (
        VisualizationTransformJob,
        VisualizationTransformResult,
    )
    from .storage import S3Storage
    from .llm_namer import LLMProvider
    from .observability import init_metrics
except ImportError:
    # Fallback to absolute imports (for direct script execution)
    from font_initializer import init_fonts_for_offline_mode
    from processor import process_visualization_job
    from models import (
        VisualizationTransformJob,
        VisualizationTransformResult,
    )
    from storage import S3Storage
    from llm_namer import LLMProvider
    from observability import init_metrics

# Initialize fonts for offline-only mode BEFORE any datamapplot usage
# This must be done early to prevent any runtime font requests
init_fonts_for_offline_mode()

# Initialize worker configuration first
WORKER_ID = os.getenv("WORKER_ID", str(uuid.uuid4()))

# Initialize observability first (before logging)
metrics = init_metrics(WORKER_ID)


# Configure structured JSON logging
def configure_json_logging(worker_id: str):
    """Configure JSON structured logging for production."""
    log_level = os.getenv("LOG_LEVEL", "INFO")
    log_format = os.getenv("LOG_FORMAT", "json").lower()

    # Create logger
    logger_instance = logging.getLogger()
    logger_instance.setLevel(getattr(logging, log_level))

    # Remove default handlers
    for handler in logger_instance.handlers[:]:
        logger_instance.removeHandler(handler)

    handler = logging.StreamHandler(sys.stdout)

    if log_format == "json":
        # JSON structured logging formatter
        class JSONFormatter(logging.Formatter):
            def format(self, record):
                log_obj = {
                    "timestamp": datetime.fromtimestamp(
                        record.created, tz=timezone.utc
                    ).isoformat(),
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
        # Human-readable format for development
        handler.setFormatter(
            logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
        )

    logger_instance.addHandler(handler)
    return logger_instance


logger = configure_json_logging(WORKER_ID)

# Configuration
NATS_URL = os.getenv("NATS_URL", "nats://localhost:4222")
NATS_SUBJECT = "workers.visualization-transform"
NATS_DURABLE_CONSUMER = "visualization-transform-workers"
# Status updates use hierarchical subjects: transforms.visualization.status.{owner}.{embedded_dataset_id}.{transform_id}
RESULT_SUBJECT_PREFIX = "transforms.visualization.status"
NATS_STREAM_RETRY_ATTEMPTS = int(os.getenv("NATS_STREAM_RETRY_ATTEMPTS", "30"))
NATS_STREAM_RETRY_DELAY = float(os.getenv("NATS_STREAM_RETRY_DELAY", "2.0"))
PROCESSING_TIMEOUT_SECS = int(os.getenv("PROCESSING_TIMEOUT_SECS", "3600"))
HEALTH_CHECK_PORT = int(os.getenv("HEALTH_CHECK_PORT", "8081"))


def build_status_subject(
    owner: str, embedded_dataset_id: int, transform_id: int
) -> str:
    """Build hierarchical NATS subject for status updates.

    Format: transforms.visualization.status.{owner}.{embedded_dataset_id}.{transform_id}
    """
    return f"{RESULT_SUBJECT_PREFIX}.{owner}.{embedded_dataset_id}.{transform_id}"


def extract_trace_context_from_headers(msg: Msg) -> dict:
    """Extract W3C trace context headers from NATS message.

    Returns a carrier dict for OpenTelemetry context extraction.
    """
    carrier = {}
    if msg.headers:
        # NATS headers are case-insensitive, but OpenTelemetry expects lowercase
        for key in ["traceparent", "tracestate"]:
            value = msg.headers.get(key)
            if value:
                carrier[key] = value
    return carrier


# Global state
s3_storage: Optional[S3Storage] = None
llm_provider: Optional[LLMProvider] = None
shutdown_event: asyncio.Event = asyncio.Event()
active_jobs = 0


async def health_check_handler(_request):
    """Handle liveness probe request."""
    return web.Response(text="OK", status=200)


async def readiness_check_handler(_request):
    """Handle readiness probe request."""

    return web.Response(text="Ready", status=200)


async def start_health_check_server():
    """Start health check HTTP server for Kubernetes probes."""
    app = web.Application()
    app.router.add_get("/health/live", health_check_handler)
    app.router.add_get("/health/ready", readiness_check_handler)

    runner = web.AppRunner(app)
    await runner.setup()
    site = web.TCPSite(runner, "0.0.0.0", HEALTH_CHECK_PORT)
    await site.start()
    logger.info(f"Health check server started on port {HEALTH_CHECK_PORT}")
    return runner


def setup_signal_handlers():
    """Setup signal handlers for graceful shutdown."""

    def handle_signal(signum, frame):
        sig_name = signal.Signals(signum).name
        logger.info(f"Received {sig_name} signal, initiating graceful shutdown")
        shutdown_event.set()

    signal.signal(signal.SIGTERM, handle_signal)
    signal.signal(signal.SIGINT, handle_signal)


async def initialize():
    """Initialize worker components."""
    global s3_storage, llm_provider

    start_time = time.time()
    logger.info(f"Initializing visualization worker {WORKER_ID}")

    # Initialize S3 storage
    s3_start = time.time()
    s3_storage = S3Storage()
    s3_elapsed = time.time() - s3_start
    metrics.s3_init_duration.observe(s3_elapsed)
    logger.info(f"S3 storage initialized in {s3_elapsed:.3f}s")

    # Initialize LLM provider
    llm_start = time.time()
    llm_provider = LLMProvider()
    llm_elapsed = time.time() - llm_start
    metrics.llm_init_duration.observe(llm_elapsed)
    logger.info(f"LLM provider initialized in {llm_elapsed:.3f}s")

    elapsed = time.time() - start_time
    logger.info(f"Worker initialization complete in {elapsed:.3f}s")

    metrics.worker_ready.set(1)


async def handle_job(
    nc: NATSConnection, msg: Msg, job: VisualizationTransformJob
) -> None:
    """
    Handle a single visualization transform job.

    Args:
        nc: NATS client
        msg: NATS message
        job: Parsed visualization transform job
    """
    global active_jobs
    active_jobs += 1
    metrics.active_jobs_gauge.set(active_jobs)
    job_start_time = time.time()

    logger.info(
        f"Processing job {job.job_id} for transform {job.visualization_transform_id} "
        f"(visualization {job.visualization_id}, owner: {job.owner_id}, embedded_dataset: {job.embedded_dataset_id})"
    )

    result = VisualizationTransformResult(
        jobId=job.job_id,
        visualizationTransformId=job.visualization_transform_id,
        visualizationId=job.visualization_id,
        ownerId=job.owner_id,
        status="processing",
    )

    # Build the status subject once for all updates in this job
    status_subject = build_status_subject(
        job.owner_id, job.embedded_dataset_id, job.visualization_transform_id
    )

    # Send immediate progress update to show job has started
    try:
        progress_result = VisualizationTransformResult(
            jobId=job.job_id,
            visualizationTransformId=job.visualization_transform_id,
            visualizationId=job.visualization_id,
            ownerId=job.owner_id,
            status="processing",
            statsJson={"stage": "starting", "progress_percent": 0},
        )
        progress_json = json.dumps(progress_result.model_dump_json_safe())
        await nc.publish(status_subject, progress_json.encode())
        logger.debug(
            f"Sent initial progress update for job {job.job_id} to {status_subject}"
        )
    except Exception as e:
        logger.warning(f"Failed to send initial progress update: {e}")

    # Define progress callback to send updates
    async def send_progress(stage: str, progress_percent: int):
        try:
            progress_result = VisualizationTransformResult(
                jobId=job.job_id,
                visualizationTransformId=job.visualization_transform_id,
                visualizationId=job.visualization_id,
                ownerId=job.owner_id,
                status="processing",
                statsJson={"stage": stage, "progress_percent": progress_percent},
            )
            progress_json = json.dumps(progress_result.model_dump_json_safe())
            await nc.publish(status_subject, progress_json.encode())
            logger.debug(
                f"Progress update for job {job.job_id}: {stage} ({progress_percent}%)"
            )
        except Exception as e:
            logger.warning(f"Failed to send progress update: {e}")

    try:
        # Process the visualization job
        process_start = time.time()
        logger.debug(f"Starting visualization processing for job {job.job_id}")
        processed_result = await asyncio.wait_for(
            process_visualization_job(
                job, llm_provider, progress_callback=send_progress
            ),
            timeout=PROCESSING_TIMEOUT_SECS,
        )
        process_elapsed = time.time() - process_start
        metrics.visualization_processing_duration.observe(process_elapsed)
        logger.info(f"Visualization processing completed in {process_elapsed:.3f}s")

        # Upload result to S3
        if s3_storage is None:
            raise RuntimeError("S3 storage not initialized")
        s3_start = time.time()
        logger.debug(f"Starting S3 upload for job {job.job_id}")
        s3_key = await s3_storage.upload_visualization(
            owner=job.owner_id,
            transform_id=job.visualization_transform_id,
            visualization_id=job.visualization_id,
            html_content=processed_result["html"],
        )
        s3_elapsed = time.time() - s3_start
        metrics.visualization_s3_upload_duration.observe(s3_elapsed)
        logger.info(f"S3 upload completed in {s3_elapsed:.3f}s")

        # Calculate processing duration
        processing_duration_ms = int((time.time() - job_start_time) * 1000)

        # Update result with success
        result.status = "success"
        result.html_s3_key = s3_key
        result.point_count = processed_result.get("point_count")
        result.cluster_count = processed_result.get("cluster_count")
        result.processing_duration_ms = processing_duration_ms
        result.stats_json = processed_result.get("stats", {})

        # Record metrics
        metrics.visualization_jobs_total.labels("success").inc()
        metrics.visualization_job_duration.observe((time.time() - job_start_time))
        metrics.visualization_points_created.inc(result.point_count or 0)
        metrics.visualization_clusters_created.inc(result.cluster_count or 0)

        job_elapsed = time.time() - job_start_time
        logger.info(
            f"Successfully completed job {job.job_id}: {result.point_count} points, "
            f"{result.cluster_count} clusters in {result.processing_duration_ms}ms "
            f"(total time: {job_elapsed:.3f}s)"
        )

    except asyncio.TimeoutError:
        result.status = "failed"
        result.error_message = f"Processing timeout after {PROCESSING_TIMEOUT_SECS}s"
        job_elapsed = time.time() - job_start_time
        metrics.visualization_jobs_total.labels("failed").inc()
        metrics.visualization_job_duration.observe(job_elapsed)
        metrics.visualization_job_failures_total.labels("timeout").inc()
        logger.error(
            f"Job {job.job_id} timeout: {result.error_message} (elapsed: {job_elapsed:.3f}s)"
        )
    except Exception as e:
        result.status = "failed"
        result.error_message = f"{type(e).__name__}: {str(e)}"
        job_elapsed = time.time() - job_start_time
        metrics.visualization_jobs_total.labels("failed").inc()
        metrics.visualization_job_duration.observe(job_elapsed)
        metrics.visualization_job_failures_total.labels(type(e).__name__).inc()
        logger.error(
            f"Job {job.job_id} failed: {result.error_message} (elapsed: {job_elapsed:.3f}s)",
            exc_info=True,
        )

    # Publish result back to Rust API
    try:
        logger.debug(f"Publishing result for job {job.job_id} to {status_subject}")
        publish_start = time.time()
        result_json = json.dumps(result.model_dump_json_safe())
        await nc.publish(status_subject, result_json.encode())
        publish_elapsed = time.time() - publish_start
        logger.info(
            f"Published result for job {job.job_id} to {status_subject} "
            f"(status: {result.status}, publish time: {publish_elapsed:.3f}s)"
        )

        # Acknowledge the message
        await msg.ack()
        metrics.nats_messages_acked_total.inc()
        logger.debug(f"Acknowledged message for job {job.job_id}")
    except Exception as e:
        logger.error(
            f"Failed to publish result for job {job.job_id}: {e}", exc_info=True
        )
        # Nack the message to retry
        await msg.nak()
        metrics.nats_messages_nacked_total.inc()
        logger.warning(f"Nacked message for job {job.job_id} for retry")
    finally:
        # Update health state and decrement active jobs
        active_jobs -= 1
        metrics.active_jobs_gauge.set(active_jobs)
        logger.debug(f"Job {job.job_id} completed, active jobs: {active_jobs}")


async def message_handler(msg: Msg, nc: NATSConnection) -> None:
    """Handle incoming NATS messages with distributed trace context."""
    handler_start = time.time()
    metrics.nats_messages_received_total.inc()

    # Extract trace context from NATS headers for distributed tracing
    carrier = extract_trace_context_from_headers(msg)
    parent_context = extract(carrier)
    tracer = trace.get_tracer(__name__)

    # Create a span with the parent context
    with tracer.start_as_current_span(
        "process_visualization_job",
        context=parent_context,
        kind=SpanKind.CONSUMER,
    ) as span:
        try:
            logger.debug("Received NATS message, parsing payload")
            # Parse the message payload
            job_data = json.loads(msg.data.decode())
            job = VisualizationTransformJob(**job_data)
            logger.debug(f"Successfully parsed job {job.job_id}")

            # Add job info to span
            span.set_attribute("job.id", str(job.job_id))
            span.set_attribute("job.transform_id", job.visualization_transform_id)
            span.set_attribute("job.visualization_id", job.visualization_id)

            # Process the job
            await handle_job(nc, msg, job)

        except ValidationError as e:
            logger.error(f"Invalid job payload: {e}", exc_info=True)
            span.set_attribute("error", True)
            span.set_attribute("error.type", "validation_error")
            # Ack the message to avoid reprocessing invalid data
            await msg.ack()
            metrics.nats_messages_acked_total.inc()
            metrics.visualization_job_failures_total.labels("validation_error").inc()
            logger.info("Acknowledged invalid message to prevent reprocessing")
        except json.JSONDecodeError as e:
            logger.error(f"Failed to parse job JSON: {e}", exc_info=True)
            span.set_attribute("error", True)
            span.set_attribute("error.type", "json_decode_error")
            await msg.ack()
            metrics.nats_messages_acked_total.inc()
            metrics.visualization_job_failures_total.labels("json_decode_error").inc()
            logger.info("Acknowledged malformed message to prevent reprocessing")
        except Exception as e:
            handler_elapsed = time.time() - handler_start
            logger.error(
                f"Unexpected error in message handler: {e} (elapsed: {handler_elapsed:.3f}s)",
                exc_info=True,
            )
            span.set_attribute("error", True)
            span.set_attribute("error.type", "unexpected_error")
            span.record_exception(e)
            await msg.nak()
            metrics.nats_messages_nacked_total.inc()
            metrics.visualization_job_failures_total.labels("unexpected_error").inc()
            logger.warning("Nacked message due to unexpected error")


async def main():
    """Main worker loop."""
    main_start = time.time()
    logger.info(
        f"Starting visualization worker (PID: {os.getpid()}, Worker ID: {WORKER_ID})"
    )

    # Setup signal handlers for graceful shutdown
    setup_signal_handlers()

    init_start = time.time()
    await initialize()
    init_elapsed = time.time() - init_start
    logger.info(f"Initialization phase completed in {init_elapsed:.3f}s")

    # Start health check server
    health_runner = None
    try:
        health_runner = await start_health_check_server()
    except Exception as e:
        logger.warning(f"Failed to start health check server: {e}")

    nc: Optional[NATSConnection] = None
    message_count = 0
    psub = None
    try:
        # Connect to NATS
        nats_start = time.time()
        logger.debug(f"Connecting to NATS at {NATS_URL}")
        nc = await nats.connect(NATS_URL)
        nats_elapsed = time.time() - nats_start
        logger.info(f"Connected to NATS at {NATS_URL} in {nats_elapsed:.3f}s")

        # Create JetStream context
        if nc is None:
            raise RuntimeError("NATS connection not established")
        js = nc.jetstream()

        # Use pull-based subscription for horizontal scaling
        # Pull subscriptions allow multiple workers to share a durable consumer
        # Retry logic handles race conditions where the stream doesn't exist yet
        # (API creates streams on startup, but workers may start first)
        psub = None
        for attempt in range(1, NATS_STREAM_RETRY_ATTEMPTS + 1):
            try:
                sub_start = time.time()
                logger.debug(
                    f"Creating pull subscriber for {NATS_SUBJECT} (attempt {attempt}/{NATS_STREAM_RETRY_ATTEMPTS})"
                )

                # First, try to bind to an existing consumer (created by API or another worker)
                # This avoids race conditions when multiple workers start simultaneously
                try:
                    psub = await js.pull_subscribe_bind(
                        consumer=NATS_DURABLE_CONSUMER,
                        stream="VISUALIZATION_TRANSFORMS",
                    )
                    sub_elapsed = time.time() - sub_start
                    logger.info(
                        f"Bound to existing consumer {NATS_DURABLE_CONSUMER} on stream VISUALIZATION_TRANSFORMS "
                        f"(worker_id: {WORKER_ID}) in {sub_elapsed:.3f}s"
                    )
                    break
                except Exception as bind_error:
                    logger.debug(f"Could not bind to existing consumer: {bind_error}, will try to create it")
                    
                    # Consumer doesn't exist yet - create it with our config
                    # Only one worker should succeed in creating; others will bind on retry
                    psub = await js.pull_subscribe(
                        subject=NATS_SUBJECT,
                        durable=NATS_DURABLE_CONSUMER,
                        config=ConsumerConfig(
                            ack_wait=1800,  # 30 minutes
                            max_deliver=3,
                            max_ack_pending=10,
                        ),
                    )
                    sub_elapsed = time.time() - sub_start
                    logger.info(
                        f"Created and subscribed to consumer {NATS_DURABLE_CONSUMER} on {NATS_SUBJECT} "
                        f"(worker_id: {WORKER_ID}) in {sub_elapsed:.3f}s"
                    )
                break  # Success, exit retry loop
            except Exception as e:
                if attempt < NATS_STREAM_RETRY_ATTEMPTS:
                    logger.warning(
                        f"Failed to subscribe to {NATS_SUBJECT} (attempt {attempt}/{NATS_STREAM_RETRY_ATTEMPTS}): {e}. "
                        f"Retrying in {NATS_STREAM_RETRY_DELAY}s... (stream may not exist yet)"
                    )
                    await asyncio.sleep(NATS_STREAM_RETRY_DELAY)
                else:
                    logger.error(
                        f"Failed to subscribe to {NATS_SUBJECT} after {NATS_STREAM_RETRY_ATTEMPTS} attempts: {e}",
                        exc_info=True,
                    )
                    raise

        if psub is None:
            raise RuntimeError(f"Failed to create subscription to {NATS_SUBJECT}")

        # Message loop with pull-based fetching
        logger.info("Worker started, waiting for jobs...")
        batch_size = int(os.getenv("NATS_BATCH_SIZE", "1"))
        fetch_timeout = float(os.getenv("NATS_FETCH_TIMEOUT", "5.0"))
        consecutive_errors = 0
        max_backoff = 30  # Maximum backoff in seconds

        while not shutdown_event.is_set():
            try:
                # Fetch messages from the pull subscription
                # This allows multiple workers to compete for messages
                messages = await psub.fetch(batch=batch_size, timeout=fetch_timeout)
                consecutive_errors = 0  # Reset on success

                for msg in messages:
                    if shutdown_event.is_set():
                        logger.info(
                            f"Shutdown requested, stopping message processing after {message_count} messages"
                        )
                        break

                    message_count += 1
                    logger.debug(f"Received message #{message_count} from queue")
                    # Create a task for message handling to allow concurrent processing
                    asyncio.create_task(message_handler(msg, nc))

            except asyncio.TimeoutError:
                # No messages available, continue polling
                consecutive_errors = 0
                continue
            except Exception as e:
                error_str = str(e).lower()
                if "timeout" in error_str:
                    # Fetch timeout is expected when no messages are available
                    consecutive_errors = 0
                    continue
                
                consecutive_errors += 1
                
                # Handle transient NATS cluster errors with exponential backoff
                if "serviceunavailable" in error_str or "no responders" in error_str:
                    backoff = min(2 ** consecutive_errors, max_backoff)
                    if consecutive_errors <= 3:
                        logger.debug(f"NATS cluster temporarily unavailable, retrying in {backoff}s...")
                    else:
                        logger.warning(f"Error fetching messages: {e}, retrying in {backoff}s (attempt {consecutive_errors})")
                    await asyncio.sleep(backoff)
                else:
                    logger.warning(f"Error fetching messages: {e}")
                    await asyncio.sleep(min(consecutive_errors, 5))  # Brief pause with mild backoff

    except KeyboardInterrupt:
        logger.info("Received shutdown signal (SIGINT)")
    except Exception as e:
        logger.error(f"Fatal error in main loop: {e}", exc_info=True)
        sys.exit(1)
    finally:
        # Gracefully wait for active jobs to complete
        if active_jobs > 0:
            logger.info(f"Waiting for {active_jobs} active jobs to complete...")
            max_wait = 300  # 5 minutes max wait for jobs
            wait_start = time.time()
            while active_jobs > 0 and (time.time() - wait_start) < max_wait:
                await asyncio.sleep(1)

            if active_jobs > 0:
                logger.warning(
                    f"{active_jobs} jobs still active after {max_wait}s timeout"
                )
            else:
                logger.info("All active jobs completed")

        # Close NATS connection
        if nc is not None:
            logger.debug("Closing NATS connection")
            await nc.close()
            logger.info("NATS connection closed")

        # Shutdown health check server
        if health_runner is not None:
            logger.debug("Shutting down health check server")
            await health_runner.cleanup()
            logger.info("Health check server shut down")

        total_elapsed = time.time() - main_start
        logger.info(
            f"Worker stopped (processed {message_count} messages, "
            f"{active_jobs} jobs still active, total runtime: {total_elapsed:.3f}s)"
        )


if __name__ == "__main__":
    asyncio.run(main())
