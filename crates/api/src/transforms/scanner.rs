use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use semantic_explorer_core::jobs::{
    EmbedderConfig, TransformFileJob, VectorDatabaseConfig, VectorEmbedJob, VisualizationConfig,
    VisualizationTransformJob,
};
use semantic_explorer_core::storage::{DocumentUpload, ensure_bucket_exists, upload_document};

use crate::storage::postgres::transforms::{get_active_transforms, get_processed_files};
use crate::storage::postgres::{collections, datasets, embedders as embedder_storage};
use crate::storage::rustfs;
use crate::transforms::models::Transform;

#[derive(Clone)]
pub(crate) struct TransformContext {
    pub(crate) postgres_pool: Pool<Postgres>,
    pub(crate) s3_client: S3Client,
}

pub(crate) async fn initialize_collection_scanner(
    postgres_pool: Pool<Postgres>,
    nats_client: NatsClient,
    s3_client: S3Client,
) -> JoinHandle<()> {
    spawn(async move {
        let mut interval = interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = scan_active_transforms(&postgres_pool, &nats_client, &s3_client).await {
                error!("Error scanning transforms: {}", e);
            }
        }
    })
}

#[tracing::instrument(name = "scan_active_transforms", skip_all)]
async fn scan_active_transforms(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
) -> Result<()> {
    let transforms = get_active_transforms(pool).await?;
    info!("Scanning {} active transforms", transforms.len());

    for transform in transforms {
        match transform.job_type.as_str() {
            "collection_to_dataset" => {
                if let Err(e) = process_collection_scan(pool, nats, s3, &transform).await {
                    error!(
                        "Failed to process collection scan for transform {}: {}",
                        transform.transform_id, e
                    );
                }
            }
            "dataset_to_vector_storage" => {
                if let Err(e) = process_vector_scan(pool, nats, s3, &transform).await {
                    error!(
                        "Failed to process vector scan for transform {}: {}",
                        transform.transform_id, e
                    );
                }
            }
            "dataset_visualization_transform" => {
                if let Err(e) = process_visualization_scan(pool, nats, &transform).await {
                    error!(
                        "Failed to process visualization scan for transform {}: {}",
                        transform.transform_id, e
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[tracing::instrument(name = "process_collection_scan", skip(pool, nats, s3, transform), fields(transform_id = %transform.transform_id))]
pub(crate) async fn process_collection_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    transform: &Transform,
) -> Result<()> {
    info!(
        "Starting collection scan for transform {}",
        transform.transform_id
    );
    let collection_id = transform
        .collection_id
        .ok_or_else(|| anyhow::anyhow!("No collection ID"))?;
    let collection = collections::get_collection(pool, &transform.owner, collection_id).await?;

    let processed = get_processed_files(pool, transform.transform_id).await?;
    let processed_keys: HashSet<String> = processed.into_iter().map(|p| p.file_key).collect();
    info!(
        "Found {} processed files for transform {}",
        processed_keys.len(),
        transform.transform_id
    );

    let mut continuation_token: Option<String> = None;
    let mut files_found = 0;
    let mut jobs_sent = 0;
    loop {
        let files =
            rustfs::list_files(s3, &collection.bucket, 100, continuation_token.as_deref()).await?;
        if files.files.is_empty() {
            break;
        }
        files_found += files.files.len();

        for file in files.files {
            if file.key.starts_with("chunks/") {
                continue;
            }

            if !processed_keys.contains(&file.key) {
                let extraction_config = transform
                    .job_config
                    .get("extraction")
                    .cloned()
                    .unwrap_or_else(|| {
                        serde_json::json!({
                            "strategy": "plain_text"
                        })
                    });

                let chunking_config = transform
                    .job_config
                    .get("chunking")
                    .cloned()
                    .unwrap_or_else(|| {
                        serde_json::json!({
                            "strategy": "sentence",
                            "chunk_size": transform.chunk_size,
                            "chunk_overlap": 0
                        })
                    });

                let embedder_config = if let Some(embedder_id) =
                    chunking_config.get("embedder_id").and_then(|v| v.as_i64())
                {
                    match embedder_storage::get_embedder(pool, &transform.owner, embedder_id as i32)
                        .await
                    {
                        Ok(embedder) => Some(EmbedderConfig {
                            provider: embedder.provider,
                            base_url: embedder.base_url,
                            api_key: embedder.api_key,
                            model: embedder
                                .config
                                .get("model")
                                .and_then(|m| m.as_str())
                                .map(|s| s.to_string()),
                            config: embedder.config,
                            max_batch_size: embedder.max_batch_size,
                        }),
                        Err(e) => {
                            error!("Failed to fetch embedder {}: {}", embedder_id, e);
                            None
                        }
                    }
                } else {
                    None
                };

                let job = TransformFileJob {
                    job_id: Uuid::new_v4(),
                    source_file_key: file.key.clone(),
                    bucket: collection.bucket.clone(),
                    transform_id: transform.transform_id,
                    extraction_config,
                    chunking_config,
                    embedder_config,
                };

                let payload = serde_json::to_vec(&job)?;
                nats.publish("workers.transform-file-worker".to_string(), payload.into())
                    .await?;
                jobs_sent += 1;
            }
        }

        if !files.has_more {
            break;
        }
        continuation_token = files.continuation_token;
    }
    info!(
        "Collection scan finished for transform {}. Found {} files, sent {} jobs.",
        transform.transform_id, files_found, jobs_sent
    );
    Ok(())
}

#[tracing::instrument(name = "process_vector_scan", skip(pool, nats, s3, transform), fields(transform_id = %transform.transform_id))]
pub(crate) async fn process_vector_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    transform: &Transform,
) -> Result<()> {
    info!(
        "Starting vector scan for transform {}",
        transform.transform_id
    );
    let owner = &transform.owner;
    let dataset_id = transform.source_dataset_id.unwrap_or(transform.dataset_id);

    let embedder_ids = transform.embedder_ids.clone().unwrap_or_default();

    info!(
        "Processing {} embedders for transform {}",
        embedder_ids.len(),
        transform.transform_id
    );

    for embedder_id in embedder_ids {
        let embedder = embedder_storage::get_embedder(pool, owner, embedder_id).await?;

        let collection_name = match transform.get_collection_name(embedder_id) {
            Some(name) => name,
            None => {
                error!(
                    "No collection mapping found for embedder {} in transform {}",
                    embedder_id, transform.transform_id
                );
                continue;
            }
        };

        info!(
            "Using collection '{}' for embedder {} in transform {}",
            collection_name, embedder_id, transform.transform_id
        );

        let items = datasets::get_dataset_items(pool, dataset_id, 0, 100).await?;
        if items.is_empty() {
            info!(
                "No items found in dataset {} for transform {}",
                dataset_id, transform.transform_id
            );
            continue;
        }

        let batch_items: Vec<serde_json::Value> = items
            .iter()
            .flat_map(|item| {
                item.chunks
                    .iter()
                    .enumerate()
                    .map(|(i, chunk)| {
                        serde_json::json!({
                            "id": Uuid::new_v4().to_string(),
                            "text": chunk.content,
                            "payload": {
                                "item_id": item.item_id,
                                "chunk_index": i,
                                "chunk_metadata": chunk.metadata,
                                "item_metadata": item.metadata
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        info!(
            "Created batch with {} chunks for embedder {} in transform {}",
            batch_items.len(),
            embedder_id,
            transform.transform_id
        );

        let batch_key = format!("batches/{}_{}.json", embedder_id, Uuid::new_v4());
        let batch_json = serde_json::to_vec(&batch_items)?;

        let bucket = collection_name.to_lowercase();

        info!(
            "Uploading batch to bucket '{}' for embedder {} in transform {}",
            bucket, embedder_id, transform.transform_id
        );

        if let Err(e) = ensure_bucket_exists(s3, &bucket).await {
            error!("Failed to ensure bucket '{}' exists: {}", bucket, e);
            return Err(e);
        }

        match upload_document(
            s3,
            DocumentUpload {
                collection_id: bucket.clone(),
                name: batch_key.clone(),
                content: batch_json,
                mime_type: "application/json".to_string(),
            },
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to upload batch to bucket '{}': {}", bucket, e);
                return Err(e);
            }
        }

        let qdrant_url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());

        let batch_size = (batch_items.len() as i32).min(embedder.max_batch_size) as usize;

        let wipe_collection = transform
            .job_config
            .get("wipe_collection")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let job = VectorEmbedJob {
            job_id: Uuid::new_v4(),
            batch_file_key: batch_key,
            bucket,
            transform_id: transform.transform_id,
            embedder_config: EmbedderConfig {
                provider: embedder.provider,
                base_url: embedder.base_url,
                api_key: embedder.api_key,
                model: embedder
                    .config
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                config: embedder.config,
                max_batch_size: embedder.max_batch_size,
            },
            vector_database_config: VectorDatabaseConfig {
                database_type: "qdrant".to_string(),
                connection_url: qdrant_url,
                api_key: None,
            },
            collection_name,
            wipe_collection,
            batch_size: Some(batch_size),
        };

        let payload = serde_json::to_vec(&job)?;
        nats.publish("workers.vector-embed-worker".to_string(), payload.into())
            .await?;

        info!(
            "Sent vector embed job for embedder {} in transform {}",
            embedder_id, transform.transform_id
        );
    }

    Ok(())
}

#[tracing::instrument(name = "process_visualization_scan", skip(pool, nats, transform), fields(transform_id = %transform.transform_id))]
pub(crate) async fn process_visualization_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    transform: &Transform,
) -> Result<()> {
    info!(
        "Starting visualization scan for transform {}",
        transform.transform_id
    );

    let owner = &transform.owner;

    // Get source_transform_id from transform
    let source_transform_id = match transform.source_transform_id {
        Some(id) => id,
        None => {
            error!(
                "No source_transform_id found for visualization transform {}",
                transform.transform_id
            );
            return Ok(());
        }
    };

    // Get source transform to find embedder and collection
    let source_transform =
        crate::storage::postgres::transforms::get_transform(pool, owner, source_transform_id)
            .await?;

    let embedder_ids = source_transform.embedder_ids.clone().unwrap_or_default();
    if embedder_ids.is_empty() {
        error!(
            "No embedders found in source transform {} for visualization transform {}",
            source_transform_id, transform.transform_id
        );
        return Ok(());
    }

    // Use first embedder (visualizations work with one embedder at a time)
    let embedder_id = embedder_ids[0];

    let source_collection = match source_transform.get_collection_name(embedder_id) {
        Some(name) => name,
        None => {
            error!(
                "No collection mapping found for embedder {} in source transform {}",
                embedder_id, source_transform_id
            );
            return Ok(());
        }
    };

    // Generate reduced collection names using the visualization transform ID
    let output_collection_reduced =
        transform.collection_name_with_suffix(embedder_id, Some("reduced"));
    let output_collection_topics =
        transform.collection_name_with_suffix(embedder_id, Some("reduced-topics"));

    // Check if visualization already exists by checking if reduced collection exists
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    let qdrant_client = if let Ok(api_key) = std::env::var("QDRANT_API_KEY") {
        qdrant_client::Qdrant::from_url(&qdrant_url)
            .api_key(api_key)
            .build()?
    } else {
        qdrant_client::Qdrant::from_url(&qdrant_url).build()?
    };

    let collection_exists = qdrant_client
        .collection_exists(&output_collection_reduced)
        .await?;

    if collection_exists {
        info!(
            "Visualization already exists for transform {} (collection: {})",
            transform.transform_id, output_collection_reduced
        );
        return Ok(());
    }

    info!(
        "Creating visualization for transform {} from source collection '{}'",
        transform.transform_id, source_collection
    );

    // Parse visualization config from job_config
    let visualization_config: VisualizationConfig =
        serde_json::from_value(transform.job_config.clone())?;

    // Create visualization job
    let job = VisualizationTransformJob {
        job_id: Uuid::new_v4(),
        transform_id: transform.transform_id,
        source_collection,
        output_collection_reduced,
        output_collection_topics,
        visualization_config,
        vector_database_config: VectorDatabaseConfig {
            database_type: "qdrant".to_string(),
            connection_url: qdrant_url,
            api_key: std::env::var("QDRANT_API_KEY").ok(),
        },
    };

    let payload = serde_json::to_vec(&job)?;
    nats.publish("workers.visualization-worker".to_string(), payload.into())
        .await?;

    info!(
        "Sent visualization job {} for transform {}",
        job.job_id, transform.transform_id
    );

    Ok(())
}
