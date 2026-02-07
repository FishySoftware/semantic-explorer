use crate::config::NatsConfig;
use anyhow::{Context, Result};
use async_nats::{
    Client, ConnectOptions, HeaderMap,
    jetstream::{
        self,
        consumer::pull::Config as ConsumerConfig,
        stream::{Config as StreamConfig, RetentionPolicy},
    },
};
use opentelemetry::{global, propagation::Injector};
use std::{cmp::min, collections::HashMap, time::Duration};
use tracing::{error, info, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Connect to NATS with automatic reconnection and exponential backoff.
/// This is the recommended way to create NATS connections for resilience.
pub async fn connect_with_retry(nats_url: &str) -> Result<Client> {
    let options = ConnectOptions::new()
        .retry_on_initial_connect()
        .max_reconnects(None) // Unlimited reconnection attempts
        .reconnect_delay_callback(|attempts| {
            // Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, 60s (max)
            let delay = min(2u64.pow(attempts.min(6) as u32), 60);
            info!("NATS reconnection attempt {}, waiting {}s", attempts, delay);
            Duration::from_secs(delay)
        })
        .connection_timeout(Duration::from_secs(10))
        .ping_interval(Duration::from_secs(30))
        .event_callback(|event| async move {
            match event {
                async_nats::Event::Connected => {
                    info!("NATS connected");
                }
                async_nats::Event::Disconnected => {
                    warn!("NATS disconnected, will attempt reconnection");
                }
                async_nats::Event::LameDuckMode => {
                    warn!("NATS server entering lame duck mode");
                }
                async_nats::Event::SlowConsumer(count) => {
                    warn!("NATS slow consumer detected: {} pending messages", count);
                }
                async_nats::Event::ServerError(err) => {
                    error!("NATS server error: {}", err);
                }
                async_nats::Event::ClientError(err) => {
                    error!("NATS client error: {}", err);
                }
                async_nats::Event::Draining => {
                    info!("NATS connection draining");
                }
                async_nats::Event::Closed => {
                    warn!("NATS connection closed");
                }
            }
        });

    let client = options
        .connect(nats_url)
        .await
        .context(format!("Failed to connect to NATS at {}", nats_url))?;

    info!(
        "Connected to NATS at {} with auto-reconnect enabled",
        nats_url
    );
    Ok(client)
}

pub async fn initialize_jetstream(client: &Client, nats_config: &NatsConfig) -> Result<()> {
    let jetstream = jetstream::new(client.clone());
    let num_replicas = nats_config.replicas as usize;

    // Stream max age in days, configurable via environment variable
    let stream_max_age_days: u64 = std::env::var("NATS_STREAM_MAX_AGE_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);
    let stream_max_age = Duration::from_secs(stream_max_age_days * 24 * 60 * 60);

    ensure_stream(
        &jetstream,
        "COLLECTION_TRANSFORMS",
        StreamConfig {
            name: "COLLECTION_TRANSFORMS".to_string(),
            subjects: vec!["workers.collection-transform".to_string()],
            retention: RetentionPolicy::WorkQueue,
            max_age: stream_max_age,
            max_messages: -1,                               // Unlimited
            duplicate_window: Duration::from_secs(60 * 60), // 60 minutes for deduplication
            num_replicas,
            ..Default::default()
        },
    )
    .await?;

    ensure_stream(
        &jetstream,
        "DATASET_TRANSFORMS",
        StreamConfig {
            name: "DATASET_TRANSFORMS".to_string(),
            subjects: vec!["workers.dataset-transform".to_string()],
            retention: RetentionPolicy::WorkQueue,
            max_age: stream_max_age,
            max_messages: -1,                               // Unlimited
            duplicate_window: Duration::from_secs(60 * 60), // 60 minutes for deduplication
            num_replicas,
            ..Default::default()
        },
    )
    .await?;

    ensure_stream(
        &jetstream,
        "VISUALIZATION_TRANSFORMS",
        StreamConfig {
            name: "VISUALIZATION_TRANSFORMS".to_string(),
            subjects: vec!["workers.visualization-transform".to_string()],
            retention: RetentionPolicy::WorkQueue,
            max_age: stream_max_age,
            max_messages: -1,                               // Unlimited
            duplicate_window: Duration::from_secs(60 * 60), // 60 minutes for deduplication
            num_replicas,
            ..Default::default()
        },
    )
    .await?;

    // Dead Letter Queue streams for failed jobs
    ensure_stream(
        &jetstream,
        "DLQ_TRANSFORMS",
        StreamConfig {
            name: "DLQ_TRANSFORMS".to_string(),
            subjects: vec![
                "dlq.collection-transforms".to_string(),
                "dlq.dataset-transforms".to_string(),
                "dlq.visualization-transforms".to_string(),
            ],
            retention: RetentionPolicy::Limits, // Keep for investigation
            max_age: Duration::from_secs(30 * 24 * 60 * 60), // 30 days for DLQ
            max_messages: -1,                   // Unlimited
            duplicate_window: Duration::from_secs(120), // 2 minutes (NATS default)
            num_replicas,
            ..Default::default()
        },
    )
    .await?;

    // Scanner trigger stream - triggers scanner workers to check for new work
    // Uses WorkQueue retention with max_ack_pending: 1 to ensure only one scanner
    // processes each trigger (HA active/standby pattern)
    ensure_stream(
        &jetstream,
        "SCANNER_TRIGGERS",
        StreamConfig {
            name: "SCANNER_TRIGGERS".to_string(),
            subjects: vec![
                "scan.trigger.collection".to_string(),
                "scan.trigger.dataset".to_string(),
                "scan.trigger.visualization".to_string(),
            ],
            retention: RetentionPolicy::WorkQueue,
            max_age: Duration::from_secs(60 * 60), // 1 hour max age
            max_messages: -1,                      // Unlimited (but limited per subject below)
            duplicate_window: Duration::from_secs(60 * 60), // 60 minutes dedup
            max_messages_per_subject: 1,           // Only one pending trigger per type
            num_replicas,
            ..Default::default()
        },
    )
    .await?;

    // Transform status stream for SSE real-time updates
    // Uses hierarchical subjects: transforms.{type}.status.{owner}.{resource_id}.{transform_id}
    // Examples:
    //   transforms.collection.status.a1b2c3d4e5f6g7h8.123.456
    //   transforms.dataset.status.a1b2c3d4e5f6g7h8.789.101
    //   transforms.visualization.status.a1b2c3d4e5f6g7h8.111.222
    // Note: owner is now a hashed username for safety with special characters
    // Wildcards allow flexible subscriptions:
    //   transforms.collection.status.> - all collection transforms
    ensure_stream(
        &jetstream,
        "TRANSFORM_STATUS",
        StreamConfig {
            name: "TRANSFORM_STATUS".to_string(),
            subjects: vec![
                "transforms.collection.status.>".to_string(),
                "transforms.dataset.status.>".to_string(),
                "transforms.visualization.status.>".to_string(),
            ],
            retention: RetentionPolicy::Limits,
            max_age: Duration::from_secs(60 * 60),
            max_messages: 100_000,
            duplicate_window: Duration::from_secs(120), // 2 minutes (NATS default)
            num_replicas,
            ..Default::default()
        },
    )
    .await?;

    info!("JetStream streams and DLQ initialized successfully");
    Ok(())
}

async fn ensure_stream(
    jetstream: &jetstream::Context,
    name: &str,
    config: StreamConfig,
) -> Result<()> {
    match jetstream.get_stream(name).await {
        Ok(mut stream) => {
            let current_config = stream.info().await?.config.clone();
            if stream_config_differs(&current_config, &config) {
                warn!(
                    "Stream '{}' exists but configuration differs. Updating stream.",
                    name
                );
                jetstream
                    .update_stream(config)
                    .await
                    .context(format!("Failed to update stream '{}'", name))?;
                info!("Stream '{}' updated successfully", name);
            } else {
                info!(
                    "Stream '{}' already exists with correct configuration",
                    name
                );
            }
        }
        Err(_) => {
            info!("Stream '{}' does not exist. Creating...", name);
            jetstream
                .create_stream(config)
                .await
                .context(format!("Failed to create stream '{}'", name))?;
            info!("Stream '{}' created successfully", name);
        }
    }
    Ok(())
}

fn stream_config_differs(current: &StreamConfig, desired: &StreamConfig) -> bool {
    if current.name != desired.name {
        tracing::debug!(
            "Stream config differs: name {:?} vs {:?}",
            current.name,
            desired.name
        );
        return true;
    }
    if current.subjects != desired.subjects {
        tracing::debug!(
            "Stream config differs: subjects {:?} vs {:?}",
            current.subjects,
            desired.subjects
        );
        return true;
    }
    if current.retention != desired.retention {
        tracing::debug!(
            "Stream config differs: retention {:?} vs {:?}",
            current.retention,
            desired.retention
        );
        return true;
    }
    if current.num_replicas != desired.num_replicas {
        tracing::debug!(
            "Stream config differs: num_replicas {:?} vs {:?}",
            current.num_replicas,
            desired.num_replicas
        );
        return true;
    }
    if current.max_age != desired.max_age {
        tracing::debug!(
            "Stream config differs: max_age {:?} vs {:?}",
            current.max_age,
            desired.max_age
        );
        return true;
    }
    if current.max_messages != desired.max_messages {
        tracing::debug!(
            "Stream config differs: max_messages {:?} vs {:?}",
            current.max_messages,
            desired.max_messages
        );
        return true;
    }
    if current.duplicate_window != desired.duplicate_window {
        tracing::debug!(
            "Stream config differs: duplicate_window {:?} vs {:?}",
            current.duplicate_window,
            desired.duplicate_window
        );
        return true;
    }
    false
}

/// Create consumer config for collection transforms with configurable parameters.
/// Falls back to environment variables or defaults if not provided.
pub fn create_transform_file_consumer_config() -> ConsumerConfig {
    let max_ack_pending = std::env::var("NATS_MAX_ACK_PENDING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let ack_wait_secs = std::env::var("NATS_COLLECTION_ACK_WAIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600); // 10 minutes default

    ConsumerConfig {
        durable_name: Some("collection-transform-workers".to_string()),
        description: Some("Consumer for file transformation jobs".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(ack_wait_secs),
        max_deliver: 5, // Retry up to 5 times
        max_ack_pending,
        backoff: vec![
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(300),
        ],
        ..Default::default()
    }
}

/// Create consumer config for dataset transforms with configurable parameters.
/// Falls back to environment variables or defaults if not provided.
pub fn create_dataset_transform_consumer_config() -> ConsumerConfig {
    let max_ack_pending = std::env::var("NATS_MAX_ACK_PENDING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let ack_wait_secs = std::env::var("NATS_DATASET_ACK_WAIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600); // 10 minutes default

    ConsumerConfig {
        durable_name: Some("dataset-transform-workers".to_string()),
        description: Some("Consumer for dataset transform embedding jobs".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(ack_wait_secs),
        max_deliver: 5, // Retry up to 5 times
        max_ack_pending,
        backoff: vec![
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(300),
        ],
        ..Default::default()
    }
}

/// Create consumer config for visualization transforms with configurable parameters.
/// Falls back to environment variables or defaults if not provided.
pub fn create_visualization_consumer_config() -> ConsumerConfig {
    // Visualization has lower max_ack_pending by default (resource-intensive)
    let max_ack_pending = std::env::var("NATS_VISUALIZATION_MAX_ACK_PENDING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let ack_wait_secs = std::env::var("NATS_VISUALIZATION_ACK_WAIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800); // 30 minutes default

    ConsumerConfig {
        durable_name: Some("visualization-transform-workers".to_string()),
        description: Some("Consumer for visualization transform jobs (UMAP/HDBSCAN)".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(ack_wait_secs),
        max_deliver: 3, // Retry up to 3 times
        max_ack_pending,
        ..Default::default()
    }
}

/// Consumer config for the scanner worker.
/// Uses max_ack_pending: 1 to ensure only one scanner processes each trigger.
/// This provides HA failover - if one scanner dies, another picks up the work.
pub fn create_scanner_consumer_config() -> ConsumerConfig {
    ConsumerConfig {
        durable_name: Some("scanner-workers".to_string()),
        description: Some("Consumer for scanner trigger messages".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(10 * 60), // 10 minutes to complete scan
        max_deliver: 3,                         // Retry up to 3 times
        max_ack_pending: 1, // Only one scanner processes triggers at a time (HA failover)
        filter_subjects: vec!["scan.trigger.>".to_string()],
        ..Default::default()
    }
}

pub async fn ensure_consumer(
    jetstream: &jetstream::Context,
    stream_name: &str,
    consumer_config: ConsumerConfig,
) -> Result<jetstream::consumer::Consumer<ConsumerConfig>> {
    let stream = jetstream
        .get_stream(stream_name)
        .await
        .context(format!("Failed to get stream '{}'", stream_name))?;

    let consumer_name = consumer_config
        .durable_name
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Consumer config must have a durable_name"))?;

    match stream.get_consumer(&consumer_name).await {
        Ok(consumer) => {
            info!(
                "Consumer '{}' already exists on stream '{}'",
                consumer_name, stream_name
            );
            Ok(consumer)
        }
        Err(_) => {
            info!(
                "Creating consumer '{}' on stream '{}'",
                consumer_name, stream_name
            );
            let consumer = stream
                .create_consumer(consumer_config)
                .await
                .context(format!(
                    "Failed to create consumer '{}' on stream '{}'",
                    consumer_name, stream_name
                ))?;
            info!("Consumer '{}' created successfully", consumer_name);
            Ok(consumer)
        }
    }
}

/// Result of a publish attempt with retry
#[derive(Debug)]
pub enum PublishResult {
    /// Message was successfully published and acknowledged
    Published,
    /// All retry attempts failed - caller should handle fallback (e.g., insert pending record)
    Failed(anyhow::Error),
}

/// Publish a message to JetStream with retry and exponential backoff.
/// Returns `PublishResult::Published` on success, or `PublishResult::Failed` after all retries.
///
/// # Arguments
/// * `client` - NATS client
/// * `subject` - NATS subject to publish to
/// * `msg_id` - Unique message ID for deduplication
/// * `payload` - Message payload bytes
/// * `max_attempts` - Maximum number of publish attempts (default: 3)
///
/// # Example
/// ```ignore
/// match publish_with_retry(&nats, "workers.dataset-transform", msg_id, payload, 3).await {
///     PublishResult::Published => info!("Job published"),
///     PublishResult::Failed(e) => {
///         // Insert pending record as fallback
///         insert_pending_record(...).await?;
///     }
/// }
/// ```
pub async fn publish_with_retry(
    client: &Client,
    subject: &str,
    msg_id: &str,
    payload: Vec<u8>,
    max_attempts: u32,
) -> PublishResult {
    let jetstream = jetstream::new(client.clone());
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        let mut headers = HeaderMap::new();
        headers.insert("Nats-Msg-Id", msg_id);
        inject_trace_context(&mut headers);

        match jetstream
            .publish_with_headers(subject.to_string(), headers, payload.clone().into())
            .await
        {
            Ok(ack_future) => {
                // Wait for acknowledgment
                match ack_future.await {
                    Ok(_) => {
                        if attempt > 1 {
                            info!("Published to {} after {} attempts", subject, attempt);
                        }
                        return PublishResult::Published;
                    }
                    Err(e) => {
                        warn!(
                            "Publish ack failed for {} (attempt {}/{}): {}",
                            subject, attempt, max_attempts, e
                        );
                        last_error = Some(anyhow::anyhow!("Ack failed: {}", e));
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Publish failed for {} (attempt {}/{}): {}",
                    subject, attempt, max_attempts, e
                );
                last_error = Some(anyhow::anyhow!("Publish failed: {}", e));
            }
        }

        // Exponential backoff before retry
        if attempt < max_attempts {
            let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
            tokio::time::sleep(delay).await;
        }
    }

    PublishResult::Failed(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown publish error")))
}

/// Start a background task to collect and export NATS metrics
pub async fn start_metrics_collector(client: Client) -> Result<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            if let Err(e) = collect_nats_metrics(&client).await {
                tracing::warn!("Failed to collect NATS metrics: {}", e);
            }
        }
    });
    Ok(())
}

async fn collect_nats_metrics(client: &Client) -> Result<()> {
    let jetstream = jetstream::new(client.clone());

    // Collect metrics for each stream
    let streams = vec![
        "COLLECTION_TRANSFORMS",
        "DATASET_TRANSFORMS",
        "VISUALIZATION_TRANSFORMS",
        "SCANNER_TRIGGERS",
        "DLQ_TRANSFORMS",
    ];

    for stream_name in streams {
        if let Ok(mut stream) = jetstream.get_stream(stream_name).await
            && let Ok(info) = stream.info().await
        {
            crate::observability::update_nats_stream_stats(
                stream_name,
                info.state.messages,
                info.state.bytes,
            );

            // Collect consumer metrics
            let consumers = vec![
                ("collection-transform-workers", "COLLECTION_TRANSFORMS"),
                ("dataset-transform-workers", "DATASET_TRANSFORMS"),
                (
                    "visualization-transform-workers",
                    "VISUALIZATION_TRANSFORMS",
                ),
                ("scanner-workers", "SCANNER_TRIGGERS"),
            ];

            for (consumer_name, expected_stream) in consumers {
                if stream_name == expected_stream
                    && let Ok(mut consumer) =
                        stream.get_consumer::<ConsumerConfig>(consumer_name).await
                    && let Ok(consumer_info) = consumer.info().await
                {
                    crate::observability::update_nats_consumer_stats(
                        stream_name,
                        consumer_name,
                        consumer_info.num_pending,
                        consumer_info.num_ack_pending as u64,
                    );
                }
            }
        }
    }

    Ok(())
}

// =============================================================================
// Trace Context Propagation (W3C Trace Context)
// =============================================================================

/// A simple injector wrapper for HashMap that implements OpenTelemetry's Injector trait
struct HashMapInjector<'a>(&'a mut HashMap<String, String>);

impl Injector for HashMapInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

/// Inject the current trace context into NATS headers.
/// Call this before publishing messages to propagate traces across service boundaries.
///
/// # Example
/// ```ignore
/// let mut headers = async_nats::HeaderMap::new();
/// headers.insert("Nats-Msg-Id", msg_id.as_str());
/// inject_trace_context(&mut headers);
/// jetstream.publish_with_headers(subject, headers, payload).await?;
/// ```
pub fn inject_trace_context(headers: &mut HeaderMap) {
    // Get current span's context
    let current_span = tracing::Span::current();
    let otel_context = current_span.context();

    // Use the global propagator to inject trace context
    let propagator = global::get_text_map_propagator(|propagator| {
        let mut carrier = HashMap::new();
        propagator.inject_context(&otel_context, &mut HashMapInjector(&mut carrier));
        carrier
    });

    // Copy from HashMap to NATS HeaderMap
    for (key, value) in propagator {
        if let Ok(header_value) = value.parse::<async_nats::HeaderValue>() {
            headers.insert(key.as_str(), header_value);
        }
    }
}

/// Extract trace context from NATS message headers.
/// Returns a HashMap containing traceparent and tracestate if present.
///
/// Use with `extract_trace_context_and_attach` to restore the context in a worker.
pub fn extract_trace_context(headers: &HeaderMap) -> HashMap<String, String> {
    let mut carrier = HashMap::new();

    // W3C Trace Context headers
    if let Some(traceparent) = headers.get("traceparent") {
        carrier.insert("traceparent".to_string(), traceparent.to_string());
    }

    if let Some(tracestate) = headers.get("tracestate") {
        carrier.insert("tracestate".to_string(), tracestate.to_string());
    }

    carrier
}

/// Extract trace context from NATS headers and return an OpenTelemetry Context.
/// The returned context can be used to create child spans.
///
/// # Example
/// ```ignore
/// let parent_context = extract_otel_context(msg.headers.as_ref());
/// // Use parent_context when creating spans
/// ```
pub fn extract_otel_context(headers: Option<&HeaderMap>) -> opentelemetry::Context {
    use opentelemetry::propagation::Extractor;

    struct HashMapExtractor<'a>(&'a HashMap<String, String>);

    impl Extractor for HashMapExtractor<'_> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).map(|s| s.as_str())
        }

        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|s| s.as_str()).collect()
        }
    }

    let carrier = headers.map(extract_trace_context).unwrap_or_default();

    global::get_text_map_propagator(|propagator| propagator.extract(&HashMapExtractor(&carrier)))
}
