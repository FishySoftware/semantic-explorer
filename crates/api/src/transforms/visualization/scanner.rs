use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::Client as NatsClient;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use semantic_explorer_core::jobs::{
    VectorDatabaseConfig, VisualizationConfig, VisualizationTransformJob,
};

use crate::storage::postgres::embedded_datasets;
use crate::storage::postgres::visualization_transforms::get_active_visualization_transforms;

/// Initialize the background scanner for visualization transforms
pub(crate) fn initialize_scanner(
    postgres_pool: Pool<Postgres>,
    nats_client: NatsClient,
) -> JoinHandle<()> {
    spawn(async move {
        let mut interval = interval(Duration::from_secs(120)); // Run less frequently
        loop {
            interval.tick().await;
            if let Err(e) = scan_active_visualization_transforms(&postgres_pool, &nats_client).await
            {
                error!("Error scanning visualization transforms: {}", e);
            }
        }
    })
}

#[tracing::instrument(name = "scan_active_visualization_transforms", skip_all)]
async fn scan_active_visualization_transforms(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
) -> Result<()> {
    let transforms = get_active_visualization_transforms(pool).await?;
    info!(
        "Scanning {} active visualization transforms",
        transforms.len()
    );

    for transform in transforms {
        if let Err(e) = process_visualization_transform_scan(pool, nats, &transform).await {
            error!(
                "Failed to process visualization transform scan for {}: {}",
                transform.visualization_transform_id, e
            );
        }
    }
    Ok(())
}

#[tracing::instrument(
    name = "process_visualization_transform_scan",
    skip(pool, nats, transform),
    fields(visualization_transform_id = %transform.visualization_transform_id)
)]
async fn process_visualization_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    transform: &crate::transforms::visualization::VisualizationTransform,
) -> Result<()> {
    info!(
        "Checking visualization transform {}",
        transform.visualization_transform_id
    );

    // Only trigger if collections don't exist yet
    if transform.reduced_collection_name.is_some() && transform.topics_collection_name.is_some() {
        info!(
            "Visualization transform {} already has output collections, skipping",
            transform.visualization_transform_id
        );
        return Ok(());
    }

    // Get the embedded dataset
    let embedded_dataset = embedded_datasets::get_embedded_dataset(
        pool,
        &transform.owner,
        transform.embedded_dataset_id,
    )
    .await?;

    // Get vector database config from environment
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let vector_db_config = VectorDatabaseConfig {
        database_type: "qdrant".to_string(),
        connection_url: qdrant_url,
        api_key: None,
    };

    // Parse visualization config or use defaults
    let visualization_config = if !transform.visualization_config.is_null() {
        serde_json::from_value::<VisualizationConfig>(transform.visualization_config.clone())
            .unwrap_or_else(|_| default_visualization_config())
    } else {
        default_visualization_config()
    };

    // Generate output collection names
    let output_collection_reduced = format!(
        "viz_reduced_{}_{}",
        transform.visualization_transform_id,
        uuid::Uuid::new_v4()
    );
    let output_collection_topics = format!(
        "viz_topics_{}_{}",
        transform.visualization_transform_id,
        uuid::Uuid::new_v4()
    );

    let job = VisualizationTransformJob {
        job_id: Uuid::new_v4(),
        visualization_transform_id: transform.visualization_transform_id,
        source_collection: embedded_dataset.collection_name.clone(),
        output_collection_reduced: output_collection_reduced.clone(),
        output_collection_topics: output_collection_topics.clone(),
        visualization_config,
        vector_database_config: vector_db_config,
    };

    let payload = serde_json::to_vec(&job)?;
    nats.publish(
        "workers.visualization-transform".to_string(),
        payload.into(),
    )
    .await?;

    info!(
        "Created visualization job for transform {}",
        transform.visualization_transform_id
    );

    Ok(())
}

fn default_visualization_config() -> VisualizationConfig {
    VisualizationConfig {
        n_neighbors: 15,
        n_components: 3,
        min_dist: 0.1,
        metric: "cosine".to_string(),
        min_cluster_size: 5,
        min_samples: Some(3),
    }
}
