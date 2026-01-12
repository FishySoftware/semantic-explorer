use crate::config::NatsConfig;
use anyhow::{Context, Result};
use async_nats::{
    Client,
    jetstream::{
        self,
        consumer::pull::Config as ConsumerConfig,
        stream::{Config as StreamConfig, RetentionPolicy},
    },
};
use std::time::Duration;
use tracing::{info, warn};

pub async fn initialize_jetstream(client: &Client, nats_config: &NatsConfig) -> Result<()> {
    let jetstream = jetstream::new(client.clone());
    let num_replicas = nats_config.replicas as usize;

    ensure_stream(
        &jetstream,
        "COLLECTION_TRANSFORMS",
        StreamConfig {
            name: "COLLECTION_TRANSFORMS".to_string(),
            subjects: vec!["workers.collection-transform".to_string()],
            retention: RetentionPolicy::WorkQueue,
            max_age: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            duplicate_window: Duration::from_secs(5 * 60),  // 5 minutes for deduplication
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
            max_age: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            duplicate_window: Duration::from_secs(5 * 60),  // 5 minutes for deduplication
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
            max_age: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            duplicate_window: Duration::from_secs(5 * 60),  // 5 minutes for deduplication
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
            max_age: Duration::from_secs(30 * 24 * 60 * 60), // 30 days
            num_replicas,
            ..Default::default()
        },
    )
    .await?;

    // Transform status stream for SSE real-time updates
    // Uses hierarchical subjects: transforms.{type}.status.{owner}.{resource_id}.{transform_id}
    // Examples:
    //   transforms.collection.status.user@example.com.123.456
    //   transforms.dataset.status.user@example.com.789.101
    //   transforms.visualization.status.user@example.com.111.222
    // Wildcards allow flexible subscriptions:
    //   transforms.collection.status.user@example.com.* - all collection transforms for user
    //   transforms.collection.status.user@example.com.123.* - transforms for specific collection
    ensure_stream(
        &jetstream,
        "TRANSFORM_STATUS",
        StreamConfig {
            name: "TRANSFORM_STATUS".to_string(),
            subjects: vec![
                "transforms.collection.status.*.*.*".to_string(),
                "transforms.dataset.status.*.*.*".to_string(),
                "transforms.visualization.status.*.*.*".to_string(),
            ],
            retention: RetentionPolicy::Limits,
            max_age: Duration::from_secs(60 * 60),
            max_messages: 100_000,
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
    current.name != desired.name
        || current.subjects != desired.subjects
        || current.retention != desired.retention
        || current.num_replicas != desired.num_replicas
}

pub fn create_transform_file_consumer_config() -> ConsumerConfig {
    ConsumerConfig {
        durable_name: Some("collection-transform-workers".to_string()),
        description: Some("Consumer for file transformation jobs".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(10 * 60), // 10 minutes to process
        max_deliver: 5,                         // Retry up to 5 times
        max_ack_pending: 100,                   // Backpressure limit
        backoff: vec![
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(300),
        ],
        ..Default::default()
    }
}

pub fn create_vector_embed_consumer_config() -> ConsumerConfig {
    ConsumerConfig {
        durable_name: Some("dataset-transform-workers".to_string()),
        description: Some("Consumer for vector embedding jobs".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(10 * 60), // 10 minutes to process
        max_deliver: 5,                         // Retry up to 5 times
        max_ack_pending: 100,                   // Backpressure limit
        backoff: vec![
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(120),
            Duration::from_secs(300),
        ],
        ..Default::default()
    }
}

pub fn create_visualization_consumer_config() -> ConsumerConfig {
    ConsumerConfig {
        durable_name: Some("visualization-transform-workers".to_string()),
        description: Some("Consumer for visualization transform jobs (UMAP/HDBSCAN)".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(30 * 60), // 30 minutes - visualization can be slow
        max_deliver: 3,                         // Retry up to 3 times
        max_ack_pending: 10,                    // Lower limit - these are resource-intensive
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
