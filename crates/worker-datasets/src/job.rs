use anyhow::Result;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, VectorParams};
use qdrant_client::qdrant::{PointStruct, UpsertPointsBuilder};
use semantic_explorer_core::models::{DatasetTransformJob, DatasetTransformResult};
use semantic_explorer_core::observability::record_worker_job;
use semantic_explorer_core::storage::get_file;
use semantic_explorer_core::validation::{validate_bucket_name, validate_s3_key};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{error, info, instrument};

use crate::embedder;

type QdrantClientCache = Arc<Mutex<HashMap<String, Qdrant>>>;

#[derive(Clone)]
pub(crate) struct WorkerContext {
    pub(crate) s3_client: aws_sdk_s3::Client,
    pub(crate) nats_client: async_nats::Client,
    /// Shared cache of Qdrant clients keyed by connection URL
    /// Enables connection reuse across jobs
    pub(crate) qdrant_cache: QdrantClientCache,
}

#[derive(serde::Deserialize)]
pub(crate) struct BatchItem {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) payload: serde_json::Map<String, serde_json::Value>,
}

#[instrument(skip(ctx), fields(job_id = %job.job_id, dataset_transform_id = %job.dataset_transform_id, embedded_dataset_id = %job.embedded_dataset_id, collection = %job.collection_name))]
pub(crate) async fn process_vector_job(job: DatasetTransformJob, ctx: WorkerContext) -> Result<()> {
    let start_time = Instant::now();
    info!("Processing vector job");

    // Validate S3 inputs to prevent path traversal attacks
    if let Err(e) = validate_bucket_name(&job.bucket) {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("vector-embed", duration, "failed_validation");
        error!(error = %e, bucket = %job.bucket, "Invalid bucket name");
        send_result(
            &ctx.nats_client,
            &job,
            Err((0, format!("Invalid bucket name: {}", e))),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        return Ok(());
    }

    if let Err(e) = validate_s3_key(&job.batch_file_key) {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("vector-embed", duration, "failed_validation");
        error!(error = %e, key = %job.batch_file_key, "Invalid S3 key");
        send_result(
            &ctx.nats_client,
            &job,
            Err((0, format!("Invalid file key: {}", e))),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        return Ok(());
    }

    // Send immediate acknowledgment that processing has started
    // This allows the frontend to show progress as the job begins
    if let Err(e) = send_progress_update(
        &ctx.nats_client,
        &job,
        0, // 0 chunks processed yet
        "processing",
        None,
    )
    .await
    {
        error!(error = %e, "Failed to send initial progress update");
        // Continue processing even if this fails
    }

    info!(batch_file = %job.batch_file_key, bucket = %job.bucket, "Downloading batch file");
    let batch_content = match get_file(&ctx.s3_client, &job.bucket, &job.batch_file_key).await {
        Ok(content) => content,
        Err(e) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("vector-embed", duration, "failed_download");
            error!(error = %e, "Failed to download batch");
            send_result(
                &ctx.nats_client,
                &job,
                Err((0, format!("Download failed: {e}"))),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
    };

    info!(
        batch_size_bytes = batch_content.len(),
        "Batch file downloaded"
    );
    let items: Vec<BatchItem> = match serde_json::from_slice(&batch_content) {
        Ok(items) => items,
        Err(e) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("vector-embed", duration, "failed_parse");
            error!(error = %e, "Failed to parse batch");
            send_result(
                &ctx.nats_client,
                &job,
                Err((0, format!("Parse failed: {e:?}"))),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
    };

    let chunk_count = items.len();
    info!(chunk_count, "Batch items parsed");

    if items.is_empty() {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("vector-embed", duration, "success_empty");
        info!("Empty batch, skipping");
        send_result(
            &ctx.nats_client,
            &job,
            Ok(0),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        return Ok(());
    }

    let texts: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
    info!(
        chunk_count,
        batch_size = job.batch_size,
        embedder_provider = ?job.embedder_config.provider,
        embedder_model = ?job.embedder_config.model,
        "Generating embeddings"
    );
    let embed_start = Instant::now();
    let embeddings = match embedder::generate_batch_embeddings(
        &job.embedder_config,
        texts,
        job.batch_size,
    )
    .await
    {
        Ok(e) => e,
        Err(e) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("vector-embed", duration, "failed_embedding");
            error!(error = %e, "Embedding failed");
            send_result(
                &ctx.nats_client,
                &job,
                Err((chunk_count, format!("Embedding failed: {e}"))),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
    };
    let embed_duration = embed_start.elapsed().as_secs_f64();
    info!(
        embedding_count = embeddings.len(),
        embed_duration_secs = embed_duration,
        "Embeddings generated successfully"
    );

    if embeddings.len() != items.len() {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("vector-embed", duration, "failed_mismatch");
        error!(
            expected = items.len(),
            actual = embeddings.len(),
            "Embedding count mismatch"
        );
        send_result(
            &ctx.nats_client,
            &job,
            Err((chunk_count, "Embedding count mismatch".to_string())),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        return Ok(());
    }

    // Get or create Qdrant client from cache for connection reuse
    let cache_key = format!(
        "{}:{}",
        job.vector_database_config.connection_url,
        job.vector_database_config.api_key.as_deref().unwrap_or("")
    );

    let qdrant_client = {
        let mut cache = ctx.qdrant_cache.lock().await;
        if let Some(client) = cache.get(&cache_key) {
            info!(qdrant_url = %job.vector_database_config.connection_url, "Reusing cached Qdrant client");
            client.clone()
        } else {
            info!(qdrant_url = %job.vector_database_config.connection_url, "Creating new Qdrant client");
            let client = Qdrant::from_url(&job.vector_database_config.connection_url)
                .api_key(job.vector_database_config.api_key.clone())
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build Qdrant client: {e}"))?;
            cache.insert(cache_key, client.clone());
            client
        }
    };

    let embedding_size = embeddings
        .first()
        .map(|e| e.len() as u64)
        .ok_or_else(|| anyhow::anyhow!("No embeddings to determine vector size"))?;

    info!(vector_size = embedding_size, "Checking collection");
    let collection_exists = qdrant_client
        .collection_info(&job.collection_name)
        .await
        .is_ok();

    if !collection_exists {
        info!(
            vector_size = embedding_size,
            distance = "Cosine",
            "Creating collection"
        );
        let create_collection = CreateCollectionBuilder::new(&job.collection_name)
            .vectors_config(VectorParams {
                size: embedding_size,
                distance: Distance::Cosine.into(),
                on_disk: Some(true), // Store vectors on disk for large collections
                ..Default::default()
            })
            .on_disk_payload(true) // Store payloads on disk to reduce memory usage
            .build();

        qdrant_client
            .create_collection(create_collection)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create collection: {}", e))?;

        info!("Collection created successfully");
    }

    let points: Vec<PointStruct> = items
        .iter()
        .zip(embeddings.into_iter())
        .map(|(item, embedding)| {
            let mut payload = item.payload.clone();
            payload.insert("text".to_string(), serde_json::json!(item.text));
            PointStruct::new(
                item.id.clone(),
                embedding,
                qdrant_client::Payload::from(payload),
            )
        })
        .collect();

    // Chunk large batches to avoid overwhelming Qdrant
    const QDRANT_CHUNK_SIZE: usize = 100;
    let point_chunks: Vec<_> = points.chunks(QDRANT_CHUNK_SIZE).collect();
    info!(
        point_count = points.len(),
        chunk_count = point_chunks.len(),
        chunk_size = QDRANT_CHUNK_SIZE,
        "Upserting points to Qdrant in chunks"
    );

    let upsert_start = Instant::now();
    for (idx, chunk) in point_chunks.into_iter().enumerate() {
        if let Err(e) = qdrant_client
            .upsert_points(
                UpsertPointsBuilder::new(&job.collection_name, chunk.to_vec()).wait(true),
            )
            .await
        {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("vector-embed", duration, "failed_upsert");
            error!(error = %e, chunk_index = idx, "Qdrant upsert chunk failed");
            return Err(anyhow::anyhow!(
                "Qdrant upsert failed at chunk {}: {}",
                idx,
                e
            ));
        }
    }
    let upsert_duration = upsert_start.elapsed().as_secs_f64();

    let duration = start_time.elapsed().as_secs_f64();
    record_worker_job("vector-embed", duration, "success");
    info!(
        duration_secs = duration,
        upsert_duration_secs = upsert_duration,
        "Job completed successfully"
    );
    send_result(
        &ctx.nats_client,
        &job,
        Ok(chunk_count),
        Some((duration * 1000.0) as i64),
    )
    .await?;

    Ok(())
}

async fn send_progress_update(
    nats: &async_nats::Client,
    job: &DatasetTransformJob,
    chunk_count: usize,
    status: &str,
    error: Option<String>,
) -> Result<()> {
    let result_msg = DatasetTransformResult {
        job_id: job.job_id,
        dataset_transform_id: job.dataset_transform_id,
        embedded_dataset_id: job.embedded_dataset_id,
        owner_id: job.owner_id.clone(),
        batch_file_key: job.batch_file_key.clone(),
        chunk_count,
        status: status.to_string(),
        error,
        processing_duration_ms: None,
    };

    let subject = semantic_explorer_core::status::dataset_status_subject(
        &job.owner_id,
        job.dataset_id,
        job.dataset_transform_id,
    );
    let payload = serde_json::to_vec(&result_msg)?;
    nats.publish(subject, payload.into()).await?;
    Ok(())
}

async fn send_result(
    nats: &async_nats::Client,
    job: &DatasetTransformJob,
    result: Result<usize, (usize, String)>,
    processing_duration_ms: Option<i64>,
) -> Result<()> {
    let (chunk_count, status, error) = match result {
        Ok(count) => (count, "success".to_string(), None),
        Err((count, e)) => (count, "failed".to_string(), Some(e)),
    };

    let result_msg = DatasetTransformResult {
        job_id: job.job_id,
        dataset_transform_id: job.dataset_transform_id,
        embedded_dataset_id: job.embedded_dataset_id,
        owner_id: job.owner_id.clone(),
        batch_file_key: job.batch_file_key.clone(),
        chunk_count,
        status,
        error,
        processing_duration_ms,
    };

    let subject = semantic_explorer_core::status::dataset_status_subject(
        &job.owner_id,
        job.dataset_id,
        job.dataset_transform_id,
    );
    let payload = serde_json::to_vec(&result_msg)?;
    nats.publish(subject, payload.into()).await?;
    Ok(())
}
