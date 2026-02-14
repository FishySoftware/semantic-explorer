use anyhow::Result;
use async_nats::Client as NatsClient;
use aws_sdk_s3::Client as S3Client;
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use tracing::{error, info, warn};
use uuid::Uuid;

use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::models::{CollectionTransformJob, EmbedderConfig};
use semantic_explorer_core::observability::record_scanner_items_discovered;

use crate::storage::postgres::collection_transforms::{
    get_active_collection_transforms_privileged, get_collection_transform_privileged,
    get_collection_transforms_for_collection, get_processed_files, is_file_already_processed,
};
use crate::storage::postgres::dataset_transform_pending_batches::{
    self as pending_batches, CreatePendingBatch,
};
use crate::storage::postgres::{collections, embedders};
use crate::storage::s3;
use crate::transforms::collection::models::CollectionTransform;

/// Resolved job configuration for a collection transform, ready to dispatch jobs.
pub(crate) struct ResolvedTransformConfig {
    pub extraction_config: serde_json::Value,
    pub chunking_config: serde_json::Value,
    pub embedder_config: Option<EmbedderConfig>,
}

/// Resolve the extraction, chunking, and embedder configs for a collection transform.
pub(crate) async fn resolve_transform_config(
    pool: &Pool<Postgres>,
    transform: &CollectionTransform,
    encryption: &EncryptionService,
) -> Result<ResolvedTransformConfig> {
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

    let embedder_config =
        get_embedder_config_for_chunking(pool, &transform.owner_id, &chunking_config, encryption)
            .await?;

    Ok(ResolvedTransformConfig {
        extraction_config,
        chunking_config,
        embedder_config,
    })
}

/// Build and dispatch a `CollectionTransformJob` for a single file.
/// Returns `true` if the job was published, `false` if it was saved for recovery.
pub(crate) async fn dispatch_file_job(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3_bucket_name: &str,
    transform: &CollectionTransform,
    config: &ResolvedTransformConfig,
    file_key: &str,
) -> Result<bool> {
    let msg_id = format!("ct-{}-{}", transform.collection_transform_id, file_key);

    let job = CollectionTransformJob {
        job_id: Uuid::new_v4(),
        source_file_key: file_key.to_string(),
        bucket: s3_bucket_name.to_string(),
        collection_id: transform.collection_id,
        collection_transform_id: transform.collection_transform_id,
        owner_id: transform.owner_id.clone(),
        extraction_config: config.extraction_config.clone(),
        chunking_config: config.chunking_config.clone(),
        embedder_config: config.embedder_config.clone(),
    };

    let payload = serde_json::to_vec(&job)?;

    match semantic_explorer_core::nats::publish_with_retry(
        nats,
        "workers.collection-transform",
        &msg_id,
        payload.clone(),
        3,
    )
    .await
    {
        semantic_explorer_core::nats::PublishResult::Published => Ok(true),
        semantic_explorer_core::nats::PublishResult::Failed(e) => {
            error!(
                "Failed to publish job for file {} after retries: {}. Saving for recovery.",
                file_key, e
            );
            if let Err(pe) = pending_batches::insert_pending_batch(
                pool,
                CreatePendingBatch {
                    batch_type: "collection".to_string(),
                    dataset_transform_id: None,
                    embedded_dataset_id: None,
                    collection_transform_id: Some(transform.collection_transform_id),
                    batch_key: file_key.to_string(),
                    s3_bucket: s3_bucket_name.to_string(),
                    job_payload: serde_json::from_slice(&payload).unwrap_or_default(),
                },
            )
            .await
            {
                error!("Failed to save pending batch for recovery: {}", pe);
            }
            Ok(false)
        }
    }
}

/// Dispatch collection transform jobs for newly uploaded files.
///
/// Called from the upload handler to immediately process uploaded files
/// through all active collection transforms for the given collection.
#[tracing::instrument(
    name = "dispatch_upload_jobs",
    skip(pool, nats, encryption),
    fields(collection_id = %collection_id, file_count = file_keys.len())
)]
pub(crate) async fn dispatch_upload_jobs(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3_bucket_name: &str,
    collection_id: i32,
    owner: &str,
    file_keys: &[String],
    encryption: &EncryptionService,
) {
    let transforms =
        match get_collection_transforms_for_collection(pool, owner, collection_id).await {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    "Failed to query collection transforms for collection {}: {}",
                    collection_id, e
                );
                return;
            }
        };

    let enabled_transforms: Vec<_> = transforms.into_iter().filter(|t| t.is_enabled).collect();
    if enabled_transforms.is_empty() {
        return;
    }

    for transform in &enabled_transforms {
        let config = match resolve_transform_config(pool, transform, encryption).await {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "Collection transform {} has invalid config: {}. Skipping.",
                    transform.collection_transform_id, e
                );
                continue;
            }
        };

        let mut jobs_sent = 0;
        for file_key in file_keys {
            match is_file_already_processed(pool, transform.collection_transform_id, file_key).await
            {
                Ok(true) => continue,
                Ok(false) => {}
                Err(e) => {
                    warn!(
                        "Failed to check processed status for {}: {}. Dispatching anyway.",
                        file_key, e
                    );
                }
            }

            match dispatch_file_job(pool, nats, s3_bucket_name, transform, &config, file_key).await
            {
                Ok(true) => jobs_sent += 1,
                Ok(false) => {} // saved for recovery
                Err(e) => error!(
                    "Failed to dispatch job for file {} on transform {}: {}",
                    file_key, transform.collection_transform_id, e
                ),
            }
        }
        if jobs_sent > 0 {
            record_scanner_items_discovered("collection", jobs_sent as u64);
            info!(
                "Dispatched {} jobs for transform {} from upload",
                jobs_sent, transform.collection_transform_id
            );
        }
    }
}

#[tracing::instrument(name = "scan_active_collection_transforms", skip_all)]
pub(crate) async fn scan_active_collection_transforms(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    encryption: &EncryptionService,
) -> Result<()> {
    let transforms = get_active_collection_transforms_privileged(pool).await?;
    info!("Scanning {} active collection transforms", transforms.len());

    for transform in transforms {
        if let Err(e) =
            backfill_collection_transform(pool, nats, s3, s3_bucket_name, &transform, encryption)
                .await
        {
            error!(
                "Failed to process collection transform scan for {}: {}",
                transform.collection_transform_id, e
            );
        }
    }
    Ok(())
}

/// Scan a specific collection transform by ID (privileged, for NATS triggers)
#[tracing::instrument(
    name = "scan_collection_transform",
    skip(pool, nats, s3, encryption),
    fields(collection_transform_id = %collection_transform_id)
)]
pub(crate) async fn scan_collection_transform(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    collection_transform_id: i32,
    encryption: &EncryptionService,
) -> Result<()> {
    let transform = get_collection_transform_privileged(pool, collection_transform_id).await?;

    if !transform.is_enabled {
        info!(
            "Collection transform {} is disabled, skipping scan",
            collection_transform_id
        );
        return Ok(());
    }

    backfill_collection_transform(pool, nats, s3, s3_bucket_name, &transform, encryption).await
}

/// Extract embedder config from chunking config if semantic chunking is used
async fn get_embedder_config_for_chunking(
    pool: &Pool<Postgres>,
    owner: &str,
    chunking_config: &serde_json::Value,
    encryption: &EncryptionService,
) -> Result<Option<EmbedderConfig>> {
    let strategy = chunking_config
        .get("strategy")
        .and_then(|v| v.as_str())
        .unwrap_or("sentence");

    if strategy != "semantic" {
        return Ok(None);
    }

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

    let embedder =
        embedders::get_embedder_by_owner_id(pool, owner, embedder_id, encryption).await?;

    let model = embedder
        .config
        .get("model")
        .and_then(|m| m.as_str())
        .ok_or_else(|| anyhow::anyhow!("Embedder config must specify a 'model' field"))?
        .to_string();

    Ok(Some(EmbedderConfig::new(
        embedder.provider,
        embedder.base_url,
        embedder.api_key,
        model,
        embedder.config,
        embedder.batch_size,
        embedder.max_input_tokens,
    )))
}

/// Backfill scan: lists all S3 files and dispatches jobs for any unprocessed ones.
/// Used on transform creation, re-enable, and as a reconciliation safety net.
#[tracing::instrument(
    name = "backfill_collection_transform",
    skip(pool, nats, s3, transform, encryption),
    fields(collection_transform_id = %transform.collection_transform_id)
)]
pub(crate) async fn backfill_collection_transform(
    pool: &Pool<Postgres>,
    nats: &NatsClient,
    s3: &S3Client,
    s3_bucket_name: &str,
    transform: &CollectionTransform,
    encryption: &EncryptionService,
) -> Result<()> {
    info!(
        "Starting backfill scan for collection transform {}",
        transform.collection_transform_id
    );

    let collection =
        collections::get_collection(pool, &transform.owner_id, transform.collection_id).await?;

    let processed = get_processed_files(pool, transform.collection_transform_id).await?;
    let processed_keys: HashSet<String> = processed.into_iter().map(|p| p.file_key).collect();
    info!(
        "Found {} processed files for collection transform {}",
        processed_keys.len(),
        transform.collection_transform_id
    );

    let config = resolve_transform_config(pool, transform, encryption).await?;

    let mut continuation_token: Option<String> = None;
    let mut files_found = 0;
    let mut jobs_sent = 0;

    loop {
        let files = s3::list_files(
            s3,
            s3_bucket_name,
            &collection.collection_id.to_string(),
            100,
            continuation_token.as_deref(),
        )
        .await?;
        if files.files.is_empty() {
            break;
        }
        files_found += files.files.len();

        for file in files.files {
            if !processed_keys.contains(&file.key) {
                match dispatch_file_job(pool, nats, s3_bucket_name, transform, &config, &file.key)
                    .await
                {
                    Ok(true) => jobs_sent += 1,
                    Ok(false) => {} // saved for recovery
                    Err(e) => error!(
                        "Failed to dispatch backfill job for file {}: {}",
                        file.key, e
                    ),
                }
            }
        }

        continuation_token = files.continuation_token;
        if continuation_token.is_none() {
            break;
        }
    }

    info!(
        "Backfill scan finished for {}. Found {} files, sent {} jobs.",
        transform.collection_transform_id, files_found, jobs_sent
    );

    if jobs_sent > 0 {
        record_scanner_items_discovered("collection", jobs_sent as u64);
    }

    Ok(())
}

/// Trigger a collection transform scan via NATS (non-blocking)
///
/// This publishes a targeted scan trigger to NATS, which will be processed
/// asynchronously by the trigger listener. This allows the API to return
/// immediately while the scan runs in the background.
#[tracing::instrument(
    name = "trigger_collection_transform_scan",
    skip(nats),
    fields(collection_transform_id = %collection_transform_id)
)]
pub async fn trigger_collection_transform_scan(
    nats: &NatsClient,
    collection_transform_id: i32,
    owner: &str,
) -> Result<()> {
    info!(
        "Publishing collection transform scan trigger for {}",
        collection_transform_id
    );

    crate::transforms::trigger::publish_targeted_trigger(
        nats,
        "collection",
        collection_transform_id,
        owner,
    )
    .await?;

    info!(
        "Published collection transform scan trigger for {}",
        collection_transform_id
    );

    Ok(())
}
