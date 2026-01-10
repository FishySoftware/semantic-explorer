use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::{Client as NatsClient, jetstream};
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

use semantic_explorer_core::models::{CollectionTransformJob, EmbedderConfig};

use crate::storage::postgres::collection_transforms::{
    get_active_collection_transforms, get_collection_transform, get_processed_files
};
use crate::storage::postgres::{collections, embedders};
use crate::storage::rustfs;
use crate::transforms::collection::models::CollectionTransform;

/// Initialize the background scanner for collection transforms
pub(crate) fn initialize_scanner(
    postgres_pool: Pool<Postgres>,
    nats_client: NatsClient,
    s3_client: S3Client,
) -> JoinHandle<()> {
    spawn(async move {
        let mut interval = interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) =
                scan_active_collection_transforms(&postgres_pool, &nats_client, &s3_client).await
            {
                error!("Error scanning collection transforms: {}", e);
            }
        }
    })
}

#[tracing::instrument(name = "scan_active_collection_transforms", skip_all)]
async fn scan_active_collection_transforms(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
) -> Result<()> {
    let transforms = get_active_collection_transforms(pool).await?;
    info!("Scanning {} active collection transforms", transforms.len());

    for transform in transforms {
        if let Err(e) = process_collection_transform_scan(pool, nats, s3, &transform).await {
            error!(
                "Failed to process collection transform scan for {}: {}",
                transform.collection_transform_id, e
            );
        }
    }
    Ok(())
}

/// Extract embedder config from chunking config if semantic chunking is used
async fn get_embedder_config_for_chunking(
    pool: &Pool<Postgres>,
    owner: &str,
    chunking_config: &serde_json::Value,
) -> Result<Option<EmbedderConfig>> {
    // Check if semantic chunking is configured
    let strategy = chunking_config
        .get("strategy")
        .and_then(|v| v.as_str())
        .unwrap_or("sentence");

    if strategy != "semantic" {
        return Ok(None);
    }

    // Get embedder_id from semantic options
    let embedder_id = chunking_config
        .get("options")
        .and_then(|opts| opts.get("semantic"))
        .and_then(|semantic| semantic.get("embedder_id"))
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let embedder_id = match embedder_id {
        Some(id) => id,
        None => {
            return Err(anyhow::anyhow!(
                "Semantic chunking requires embedder_id in chunking config"
            ));
        }
    };

    // Fetch the embedder
    let embedder = embedders::get_embedder(pool, owner, embedder_id).await?;

    Ok(Some(EmbedderConfig {
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
        max_input_tokens: embedder.max_input_tokens,
    }))
}

#[tracing::instrument(
    name = "process_collection_transform_scan",
    skip(pool, nats, s3, transform),
    fields(collection_transform_id = %transform.collection_transform_id)
)]
async fn process_collection_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    transform: &CollectionTransform,
) -> Result<()> {
    info!(
        "Starting collection scan for collection transform {}",
        transform.collection_transform_id
    );

    let collection =
        collections::get_collection(pool, &transform.owner, transform.collection_id).await?;

    // Get already processed files
    let processed = get_processed_files(pool, transform.collection_transform_id).await?;
    let processed_keys: HashSet<String> = processed.into_iter().map(|p| p.file_key).collect();
    info!(
        "Found {} processed files for collection transform {}",
        processed_keys.len(),
        transform.collection_transform_id
    );

    // Build extraction and chunking configs
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

    // Get embedder config if semantic chunking is used
    let embedder_config =
        match get_embedder_config_for_chunking(pool, &transform.owner, &chunking_config).await {
            Ok(config) => config,
            Err(e) => {
                // Record config error - this affects all files
                warn!(
                    "Collection transform {} has invalid chunking config: {}. Skipping.",
                    transform.collection_transform_id, e
                );
                return Err(e);
            }
        };

    let mut continuation_token: Option<String> = None;
    let mut files_found = 0;
    let mut jobs_sent = 0;

    // Iterate through all files in the collection bucket
    loop {
        let files =
            rustfs::list_files(s3, &collection.bucket, 100, continuation_token.as_deref()).await?;
        if files.files.is_empty() {
            break;
        }
        files_found += files.files.len();

        for file in files.files {
            // Skip chunk files (outputs from previous transforms)
            if file.key.starts_with("chunks/") {
                continue;
            }

            // Skip already processed files
            if !processed_keys.contains(&file.key) {
                let msg_id = format!("ct-{}-{}", transform.collection_transform_id, file.key);

                let job = CollectionTransformJob {
                    job_id: Uuid::new_v4(),
                    source_file_key: file.key.clone(),
                    bucket: collection.bucket.clone(),
                    collection_transform_id: transform.collection_transform_id,
                    owner: transform.owner.clone(),
                    extraction_config: extraction_config.clone(),
                    chunking_config: chunking_config.clone(),
                    embedder_config: embedder_config.clone(),
                };

                let payload = serde_json::to_vec(&job)?;

                // Use JetStream publish with message ID for deduplication
                let jetstream = jetstream::new(nats.clone());
                let mut headers = async_nats::HeaderMap::new();
                headers.insert("Nats-Msg-Id", msg_id.as_str());

                jetstream
                    .publish_with_headers(
                        "workers.collection-transform".to_string(),
                        headers,
                        payload.into(),
                    )
                    .await?
                    .await?; // Wait for ack
                jobs_sent += 1;
            }
        }

        if !files.has_more {
            break;
        }
        continuation_token = files.continuation_token;
    }

    info!(
        "Collection transform scan finished for {}. Found {} files, sent {} jobs.",
        transform.collection_transform_id, files_found, jobs_sent
    );

    Ok(())
}

/// Trigger a collection transform scan immediately
#[tracing::instrument(
    name = "trigger_collection_transform_scan",
    skip(pool, nats, s3),
    fields(collection_transform_id = %collection_transform_id)
)]
pub async fn trigger_collection_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    collection_transform_id: i32,
    owner: &str,
) -> Result<()> {
    info!(
        "Triggering collection transform scan for {}",
        collection_transform_id
    );

    // Get the collection transform
    let transform = get_collection_transform(
        pool,
        owner,
        collection_transform_id,
    )
    .await?;

    // Process the scan immediately
    process_collection_transform_scan(pool, nats, s3, &transform).await?;

    info!(
        "Triggered collection transform scan for {}",
        collection_transform_id
    );

    Ok(())
}
