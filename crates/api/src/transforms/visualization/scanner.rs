use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::Client as NatsClient;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use semantic_explorer_core::models::{
    VectorDatabaseConfig, VisualizationConfig, VisualizationTransformJob,
};

use crate::storage::postgres::embedded_datasets;
use crate::storage::postgres::visualization_transforms::{
    get_active_visualization_transforms, update_visualization_transform_status_processing,
};

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

    // Only trigger if collections don't exist yet (collection names are null in DB)
    if transform.reduced_collection_name.is_some() {
        info!(
            "Visualization transform {} already has output collection name set, skipping",
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

    // Generate output collection names using the standard naming convention
    let (output_collection_reduced, output_collection_topics) =
        crate::transforms::visualization::VisualizationTransform::generate_collection_names(
            transform.visualization_transform_id,
            &transform.owner,
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

    // Mark the transform as processing before publishing the job
    if let Err(e) =
        update_visualization_transform_status_processing(pool, transform.visualization_transform_id)
            .await
    {
        error!(
            "Failed to update processing status for visualization transform {}: {}",
            transform.visualization_transform_id, e
        );
    }

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
        min_dist: 0.25,
        metric: "cosine".to_string(),
        min_cluster_size: 15,
        min_samples: Some(5),
        topic_naming_mode: "tfidf".to_string(),
        topic_naming_llm_id: None,
    }
}

/// Trigger a visualization transform job immediately
#[tracing::instrument(
    name = "trigger_visualization_transform_job",
    skip(pool, nats),
    fields(visualization_transform_id = %visualization_transform_id)
)]
pub async fn trigger_visualization_transform_job(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    visualization_transform_id: i32,
    owner: &str,
) -> Result<()> {
    info!(
        "Triggering visualization transform job for {}",
        visualization_transform_id
    );

    // Get the visualization transform
    let transform =
        crate::storage::postgres::visualization_transforms::get_visualization_transform(
            pool,
            owner,
            visualization_transform_id,
        )
        .await?;

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

    // Generate output collection names using the standard naming convention
    let (output_collection_reduced, output_collection_topics) =
        crate::transforms::visualization::VisualizationTransform::generate_collection_names(
            transform.visualization_transform_id,
            &transform.owner,
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

    // Mark the transform as processing before publishing the job
    update_visualization_transform_status_processing(pool, transform.visualization_transform_id)
        .await?;

    let payload = serde_json::to_vec(&job)?;
    nats.publish(
        "workers.visualization-transform".to_string(),
        payload.into(),
    )
    .await?;

    info!(
        "Triggered visualization job for transform {}",
        transform.visualization_transform_id
    );

    Ok(())
}
