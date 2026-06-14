use anyhow::Result;
use async_nats::jetstream;
use futures_util::stream::{self, StreamExt};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::PointStruct;
use qdrant_client::qdrant::{Distance, UpsertPointsBuilder};
use semantic_explorer_core::embedder;
use semantic_explorer_core::models::{DatasetTransformJob, DatasetTransformResult, EmbedderConfig};
use semantic_explorer_core::nats::inject_trace_context;
use semantic_explorer_core::observability::record_worker_job;
use semantic_explorer_core::storage::get_file_with_size_check;
use semantic_explorer_core::validation::{validate_bucket_name, validate_s3_key};
use semantic_explorer_core::worker::WorkerContext;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tracing::{error, info, instrument, warn};

const QDRANT_CHUNK_SIZE: usize = 1000;

/// Default number of upsert passes over the set of failing chunks before giving up.
pub const DEFAULT_UPSERT_MAX_ATTEMPTS: u32 = 4;

/// Cached parallel upload count (set once from main via init_job_config)
static QDRANT_PARALLEL_UPLOADS: OnceLock<usize> = OnceLock::new();

/// Cached upsert max attempts (set once from main via init_job_config)
static UPSERT_MAX_ATTEMPTS: OnceLock<u32> = OnceLock::new();

/// Initialize job-level configuration. Call once from main.
pub fn init_job_config(qdrant_parallel_uploads: usize, upsert_max_attempts: u32) {
    QDRANT_PARALLEL_UPLOADS.get_or_init(|| qdrant_parallel_uploads);
    UPSERT_MAX_ATTEMPTS.get_or_init(|| upsert_max_attempts);
}

#[derive(serde::Deserialize)]
pub(crate) struct BatchItem {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) payload: serde_json::Map<String, serde_json::Value>,
}

#[instrument(skip(ctx), fields(job_id = %job.job_id, dataset_transform_id = %job.dataset_transform_id, embedded_dataset_id = %job.embedded_dataset_id, collection = %job.collection_name))]
pub(crate) async fn process_dataset_transform_job(
    job: DatasetTransformJob,
    ctx: WorkerContext,
) -> Result<()> {
    let start_time = Instant::now();
    info!("Processing dataset transform job");

    // Validate S3 inputs to prevent path traversal attacks
    if let Err(e) = validate_bucket_name(&job.bucket) {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("dataset-transform", duration, "failed_validation");
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
        record_worker_job("dataset-transform", duration, "failed_validation");
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
    let batch_content =
        match get_file_with_size_check(&ctx.s3_client, &job.bucket, &job.batch_file_key).await {
            Ok(content) => content,
            Err(e) => {
                let duration = start_time.elapsed().as_secs_f64();
                record_worker_job("dataset-transform", duration, "failed_download");
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
            record_worker_job("dataset-transform", duration, "failed_parse");
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
        record_worker_job("dataset-transform", duration, "success_empty");
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

    // Pre-embedding abort check: verify batch file still exists in S3.
    // If the transform was deleted, the API cleans up S3 batch files so workers
    // can detect deletion and abort BEFORE wasting embedding tokens.
    match semantic_explorer_core::storage::file_exists(
        &ctx.s3_client,
        &job.bucket,
        &job.batch_file_key,
    )
    .await
    {
        Ok(false) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("dataset-transform", duration, "aborted_deleted");
            warn!(
                batch_file = %job.batch_file_key,
                dataset_transform_id = job.dataset_transform_id,
                "Batch file deleted from S3 — transform likely cancelled. Aborting before embedding."
            );
            send_result(
                &ctx.nats_client,
                &job,
                Err((
                    0,
                    "Batch file no longer exists (transform deleted)".to_string(),
                )),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
        Ok(true) => { /* File still exists, proceed normally */ }
        Err(e) => {
            // HEAD request failed for a reason other than NotFound — log and proceed
            // to avoid false-positive aborts on transient S3 issues
            warn!(
                error = %e,
                batch_file = %job.batch_file_key,
                "Failed to verify batch file existence, proceeding with embedding"
            );
        }
    }

    info!(
        chunk_count,
        batch_size = job.batch_size,
        embedder_provider = ?job.embedder_config.provider,
        embedder_model = ?job.embedder_config.model,
        collection_name = %job.collection_name,
        embedded_dataset_id = job.embedded_dataset_id,
        "Generating embeddings"
    );

    let embeddings = match embedder::generate_batch_embeddings(
        &job.embedder_config,
        texts,
        job.batch_size,
    )
    .await
    {
        Ok(embeddings) => embeddings,
        Err(e) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("dataset-transform", duration, "failed_embedding");
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
    info!(
        embedding_count = embeddings.len(),
        "Embeddings generated successfully"
    );

    if embeddings.len() != items.len() {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("dataset-transform", duration, "failed_mismatch");
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

    // Get cached Qdrant client instead of recreating for each job
    let qdrant_client = crate::qdrant_cache::get_or_create_client(
        &job.qdrant_config.url,
        job.qdrant_config
            .api_key
            .as_ref()
            .map(|k| k.expose_secret().to_string()),
    )?;

    let embedding_size = embeddings
        .first()
        .map(|e| e.len() as u64)
        .ok_or_else(|| anyhow::anyhow!("No embeddings to determine vector size"))?;

    // Ensure collection exists using cached check (avoids redundant API calls)
    crate::qdrant_cache::ensure_collection_exists(
        &qdrant_client,
        &job.qdrant_config.url,
        &job.collection_name,
        embedding_size,
        distance_from_embedder_config(&job.embedder_config),
    )
    .await?;

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

    let point_chunks: Vec<Vec<PointStruct>> = points
        .chunks(QDRANT_CHUNK_SIZE)
        .map(|c| c.to_vec())
        .collect();

    // Get parallel upload count from config (default 4)
    let parallel_uploads: usize = QDRANT_PARALLEL_UPLOADS.get().copied().unwrap_or(4);

    info!(
        point_count = points.len(),
        chunk_count = point_chunks.len(),
        chunk_size = QDRANT_CHUNK_SIZE,
        parallel_uploads = parallel_uploads,
        "Upserting points to Qdrant in parallel chunks"
    );

    // Report progress now that embeddings are ready and upsert is starting, so the
    // frontend sees the real total ahead of the (potentially long) upsert phase.
    if let Err(e) =
        send_progress_update(&ctx.nats_client, &job, chunk_count, "processing", None).await
    {
        error!(error = %e, "Failed to send pre-upsert progress update");
    }

    let upsert_start = Instant::now();
    let max_upsert_attempts: u32 = UPSERT_MAX_ATTEMPTS
        .get()
        .copied()
        .unwrap_or(DEFAULT_UPSERT_MAX_ATTEMPTS);

    if let Err(e) = upsert_chunks_with_retry(
        &qdrant_client,
        &job.collection_name,
        &point_chunks,
        parallel_uploads,
        max_upsert_attempts,
    )
    .await
    {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("dataset-transform", duration, "failed_upsert");
        error!(error = %e, "Qdrant upsert failed after retries");
        send_result(
            &ctx.nats_client,
            &job,
            Err((chunk_count, format!("Qdrant upsert failed: {e}"))),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        // Return Err so NATS redelivers as a last resort: point IDs are deterministic,
        // making re-upsert idempotent (no duplicates).
        return Err(anyhow::anyhow!("Qdrant upsert failed: {e}"));
    }
    let upsert_duration = upsert_start.elapsed().as_secs_f64();

    let duration = start_time.elapsed().as_secs_f64();
    record_worker_job("dataset-transform", duration, "success");
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

/// Derive the collection distance metric from the embedder config.
///
/// Reads an optional `distance` field from the embedder's `config` object, defaulting
/// to cosine (the convention for normalized embeddings). Unknown values fall back to
/// cosine with a warning rather than failing the job.
fn distance_from_embedder_config(config: &EmbedderConfig) -> Distance {
    let configured = config
        .config
        .get("distance")
        .and_then(|v| v.as_str())
        .map(str::to_ascii_lowercase);

    match configured.as_deref() {
        None | Some("cosine") => Distance::Cosine,
        Some("dot" | "dotproduct" | "dot_product") => Distance::Dot,
        Some("euclid" | "euclidean" | "l2") => Distance::Euclid,
        Some("manhattan" | "l1") => Distance::Manhattan,
        Some(other) => {
            warn!(
                distance = other,
                "Unknown distance metric in embedder config, defaulting to Cosine"
            );
            Distance::Cosine
        }
    }
}

/// Upsert all chunks with bounded concurrency, retrying only the chunks that fail.
///
/// Embeddings are the expensive resource, so retrying failed chunks in-process (the
/// vectors are already in memory) avoids the download + re-embed cost of a full job
/// redelivery. Retries apply only to transient errors; a non-retryable failure aborts
/// immediately. On success every chunk has been upserted.
async fn upsert_chunks_with_retry(
    client: &Arc<Qdrant>,
    collection: &str,
    chunks: &[Vec<PointStruct>],
    parallel_uploads: usize,
    max_attempts: u32,
) -> Result<()> {
    let total = chunks.len();
    let mut pending: Vec<usize> = (0..total).collect();

    for attempt in 1..=max_attempts {
        let results: Vec<(usize, Option<(String, bool)>)> = stream::iter(pending.iter().copied())
            .map(|idx| {
                let client = Arc::clone(client);
                let collection = collection.to_string();
                let chunk = chunks[idx].clone();
                async move {
                    match client
                        .upsert_points(UpsertPointsBuilder::new(&collection, chunk).wait(true))
                        .await
                    {
                        Ok(_) => (idx, None),
                        Err(e) => {
                            let retryable = crate::qdrant_cache::is_retryable(&e);
                            (idx, Some((e.to_string(), retryable)))
                        }
                    }
                }
            })
            .buffer_unordered(parallel_uploads)
            .collect()
            .await;

        let mut failed = Vec::new();
        let mut fatal: Option<(usize, String)> = None;
        for (idx, outcome) in results {
            if let Some((err, retryable)) = outcome {
                if !retryable && fatal.is_none() {
                    fatal = Some((idx, err.clone()));
                }
                failed.push((idx, err));
            }
        }

        if failed.is_empty() {
            return Ok(());
        }

        if let Some((idx, err)) = fatal {
            return Err(anyhow::anyhow!(
                "non-retryable error at chunk {idx} ({}/{total} chunks failed): {err}",
                failed.len()
            ));
        }

        if attempt == max_attempts {
            let (idx, err) = &failed[0];
            return Err(anyhow::anyhow!(
                "{}/{total} chunks still failing after {max_attempts} attempts; first failure at chunk {idx}: {err}",
                failed.len()
            ));
        }

        let delay = Duration::from_secs(1u64 << (attempt - 1).min(4));
        warn!(
            attempt,
            max_attempts,
            succeeded = total - failed.len(),
            failed = failed.len(),
            delay_secs = delay.as_secs(),
            "Retrying failed Qdrant upsert chunks"
        );
        tokio::time::sleep(delay).await;
        pending = failed.into_iter().map(|(idx, _)| idx).collect();
    }

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

    // Use JetStream publish with acknowledgment for reliable delivery
    let js = jetstream::new(nats.clone());
    let mut headers = async_nats::HeaderMap::new();
    let msg_id = format!(
        "prog-{}-{}-{}-{}",
        job.job_id, job.batch_file_key, status, chunk_count
    );
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    inject_trace_context(&mut headers);

    match js
        .publish_with_headers(subject.clone(), headers, payload.into())
        .await
    {
        Ok(ack_future) => {
            if let Err(e) = ack_future.await {
                warn!(
                    "JetStream ack failed for progress update on {}: {}",
                    subject, e
                );
                // Progress updates are non-critical, log and continue
            }
        }
        Err(e) => {
            warn!("Failed to publish progress update to {}: {}", subject, e);
            // Progress updates are non-critical, log and continue
        }
    }

    Ok(())
}

/// Send result with guaranteed delivery using JetStream.
/// This is critical for accurate stats tracking - results MUST be delivered reliably.
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
        status: status.clone(),
        error,
        processing_duration_ms,
    };

    let subject = semantic_explorer_core::status::dataset_status_subject(
        &job.owner_id,
        job.dataset_id,
        job.dataset_transform_id,
    );
    let payload = serde_json::to_vec(&result_msg)?;

    // Use JetStream publish with acknowledgment for GUARANTEED delivery
    // This is critical - if the result isn't delivered, stats will be incorrect
    let js = jetstream::new(nats.clone());
    let mut headers = async_nats::HeaderMap::new();
    // Use deterministic message ID for deduplication
    let msg_id = format!("result-{}-{}-{}", job.job_id, job.batch_file_key, status);
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    inject_trace_context(&mut headers);

    // Retry up to 3 times with backoff for critical results
    let mut last_error = None;
    for attempt in 1..=3 {
        match js
            .publish_with_headers(subject.clone(), headers.clone(), payload.clone().into())
            .await
        {
            Ok(ack_future) => match ack_future.await {
                Ok(_) => {
                    if attempt > 1 {
                        info!("Result published to {} after {} attempts", subject, attempt);
                    }
                    return Ok(());
                }
                Err(e) => {
                    warn!(
                        "JetStream ack failed for result on {} (attempt {}/3): {}",
                        subject, attempt, e
                    );
                    last_error = Some(anyhow::anyhow!("Ack failed: {}", e));
                }
            },
            Err(e) => {
                warn!(
                    "Failed to publish result to {} (attempt {}/3): {}",
                    subject, attempt, e
                );
                last_error = Some(anyhow::anyhow!("Publish failed: {}", e));
            }
        }

        // Exponential backoff before retry
        if attempt < 3 {
            tokio::time::sleep(std::time::Duration::from_millis(
                100 * 2u64.pow(attempt - 1),
            ))
            .await;
        }
    }

    // If all retries failed, this is a critical error
    error!(
        "CRITICAL: Failed to publish result to {} after 3 attempts. Stats will be incorrect!",
        subject
    );
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown publish error")))
}
