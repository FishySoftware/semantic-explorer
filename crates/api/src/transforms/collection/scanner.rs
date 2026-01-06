use actix_web::rt::{spawn, task::JoinHandle, time::interval};
use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use semantic_explorer_core::jobs::CollectionTransformJob;

use crate::storage::postgres::collection_transforms::{
    get_active_collection_transforms, get_processed_files,
};
use crate::storage::postgres::collections;
use crate::storage::rustfs;

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

#[tracing::instrument(
    name = "process_collection_transform_scan",
    skip(pool, nats, s3, transform),
    fields(collection_transform_id = %transform.collection_transform_id)
)]
async fn process_collection_transform_scan(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    transform: &crate::transforms::collection::CollectionTransform,
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

                let job = CollectionTransformJob {
                    job_id: Uuid::new_v4(),
                    source_file_key: file.key.clone(),
                    bucket: collection.bucket.clone(),
                    collection_transform_id: transform.collection_transform_id,
                    extraction_config,
                    chunking_config,
                };

                let payload = serde_json::to_vec(&job)?;
                nats.publish("workers.collection-transform".to_string(), payload.into())
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
        "Collection transform scan finished for {}. Found {} files, sent {} jobs.",
        transform.collection_transform_id, files_found, jobs_sent
    );

    Ok(())
}
