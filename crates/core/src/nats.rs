use anyhow::{Context, Result};
use async_nats::{
    jetstream::{
        self,
        consumer::pull::Config as ConsumerConfig,
        stream::{Config as StreamConfig, RetentionPolicy},
    },
    Client,
};
use std::time::Duration;
use tracing::{info, warn};

pub async fn initialize_jetstream(client: &Client) -> Result<()> {
    let jetstream = jetstream::new(client.clone());

    ensure_stream(
        &jetstream,
        "COLLECTION_TRANSFORMS",
        StreamConfig {
            name: "COLLECTION_TRANSFORMS".to_string(),
            subjects: vec!["workers.collection-transform".to_string()],
            retention: RetentionPolicy::WorkQueue,
            max_age: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            duplicate_window: Duration::from_secs(5 * 60),  // 5 minutes for deduplication
            num_replicas: 1,
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
            num_replicas: 1,
            ..Default::default()
        },
    )
    .await?;

    ensure_stream(
        &jetstream,
        "VISUALIZATION_TRANSFORMS",
        StreamConfig {
            name: "VISUALIZATION_TRANSFORMS".to_string(),
            subjects: vec!["workers.visualization-worker".to_string()],
            retention: RetentionPolicy::WorkQueue,
            max_age: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            duplicate_window: Duration::from_secs(5 * 60),  // 5 minutes for deduplication
            num_replicas: 1,
            ..Default::default()
        },
    )
    .await?;

    info!("JetStream streams initialized successfully");
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
        durable_name: Some("transform-file-workers".to_string()),
        description: Some("Consumer for file transformation jobs".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(10 * 60), // 10 minutes to process
        max_deliver: 5,                         // Retry up to 5 times
        max_ack_pending: 100,                   // Backpressure limit
        ..Default::default()
    }
}

pub fn create_vector_embed_consumer_config() -> ConsumerConfig {
    ConsumerConfig {
        durable_name: Some("vector-embed-workers".to_string()),
        description: Some("Consumer for vector embedding jobs".to_string()),
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(10 * 60), // 10 minutes to process
        max_deliver: 5,                         // Retry up to 5 times
        max_ack_pending: 100,                   // Backpressure limit
        ..Default::default()
    }
}

pub fn create_visualization_consumer_config() -> ConsumerConfig {
    ConsumerConfig {
        durable_name: Some("visualization-workers".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_differs() {
        let config1 = StreamConfig {
            name: "TEST".to_string(),
            subjects: vec!["test.subject".to_string()],
            retention: RetentionPolicy::WorkQueue,
            num_replicas: 1,
            ..Default::default()
        };

        let config2 = config1.clone();
        assert!(!stream_config_differs(&config1, &config2));

        let config3 = StreamConfig {
            name: "TEST".to_string(),
            subjects: vec!["different.subject".to_string()],
            retention: RetentionPolicy::WorkQueue,
            num_replicas: 1,
            ..Default::default()
        };
        assert!(stream_config_differs(&config1, &config3));
    }
}
