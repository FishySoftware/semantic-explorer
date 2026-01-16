use anyhow::Result;
use semantic_explorer_core::models::{CollectionTransformJob, CollectionTransformResult};
use semantic_explorer_core::observability::record_worker_job;
use semantic_explorer_core::storage::{DocumentUpload, get_file_with_size_check, upload_document};
use semantic_explorer_core::validation::{validate_bucket_name, validate_s3_key};
use std::{env, time::Instant};
use tracing::{error, info, instrument};

use crate::chunk::{ChunkingService, config::ChunkingConfig};
use crate::extract::{ExtractionService, config::ExtractionConfig};

#[derive(Clone)]
pub(crate) struct WorkerContext {
    pub(crate) s3_client: aws_sdk_s3::Client,
    pub(crate) nats_client: async_nats::Client,
}

#[instrument(skip(ctx), fields(job_id = %job.job_id, collection_transform_id = %job.collection_transform_id, file = %job.source_file_key))]
pub(crate) async fn process_file_job(
    job: CollectionTransformJob,
    ctx: WorkerContext,
) -> Result<()> {
    let start_time = Instant::now();

    // Get the actual S3 bucket name from environment
    let s3_bucket_name = match env::var("S3_BUCKET_NAME") {
        Ok(bucket) => bucket,
        Err(_) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("transform-file", duration, "failed_config");
            error!("S3_BUCKET_NAME environment variable not set");
            send_result(
                &ctx.nats_client,
                &job,
                Err("S3_BUCKET_NAME environment variable not set".to_string()),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
    };

    // Validate S3 inputs to prevent path traversal attacks
    if let Err(e) = validate_bucket_name(&s3_bucket_name) {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("transform-file", duration, "failed_validation");
        error!(error = %e, bucket = %s3_bucket_name, "Invalid bucket name");
        send_result(
            &ctx.nats_client,
            &job,
            Err(format!("Invalid bucket name: {}", e)),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        return Ok(());
    }

    // Construct the full S3 key: collections/{collection_id}/{filename}
    let full_source_key = format!("collections/{}/{}", job.collection_id, job.source_file_key);

    if let Err(e) = validate_s3_key(&full_source_key) {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("transform-file", duration, "failed_validation");
        error!(error = %e, key = %full_source_key, "Invalid S3 key");
        send_result(
            &ctx.nats_client,
            &job,
            Err(format!("Invalid file key: {}", e)),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        return Ok(());
    }

    info!(bucket = %s3_bucket_name, key = %full_source_key, "Downloading file");
    let file_content =
        match get_file_with_size_check(&ctx.s3_client, &s3_bucket_name, &full_source_key).await {
            Ok(content) => content,
            Err(e) => {
                let duration = start_time.elapsed().as_secs_f64();
                let error_msg = e.to_string();

                // Check if this is a file size error
                if error_msg.contains("exceeds maximum limit") {
                    record_worker_job("transform-file", duration, "failed_file_too_large");
                    error!(error = %e, "File exceeds size limit");
                } else {
                    record_worker_job("transform-file", duration, "failed_download");
                    error!(error = %e, "Failed to download file");
                }

                send_result(
                    &ctx.nats_client,
                    &job,
                    Err(error_msg),
                    Some((duration * 1000.0) as i64),
                )
                .await?;
                return Ok(());
            }
        };
    info!(
        file_size_bytes = file_content.len(),
        "Downloaded file successfully"
    );

    let extraction_config: ExtractionConfig =
        match serde_json::from_value(job.extraction_config.clone()) {
            Ok(config) => config,
            Err(e) => {
                let duration = start_time.elapsed().as_secs_f64();
                record_worker_job("transform-file", duration, "failed_config_parse");
                error!(error = %e, "Failed to parse extraction config");
                send_result(
                    &ctx.nats_client,
                    &job,
                    Err(format!("Invalid extraction config: {}", e)),
                    Some((duration * 1000.0) as i64),
                )
                .await?;
                return Ok(());
            }
        };

    let mime_type = mime_guess::from_path(&job.source_file_key).first_or_octet_stream();
    info!(
        mime_type = %mime_type,
        strategy = ?extraction_config.strategy,
        "Extracting text"
    );

    let extraction_result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ExtractionService::extract(&mime_type, &file_content, &extraction_config)
    })) {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("transform-file", duration, "failed_extraction");
            error!(error = %e, mime_type = %mime_type, "Extraction failed");
            send_result(
                &ctx.nats_client,
                &job,
                Err(e.to_string()),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
        Err(_) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("transform-file", duration, "failed_extraction");
            error!(mime_type = %mime_type, "Extraction panicked");
            send_result(
                &ctx.nats_client,
                &job,
                Err("Extraction panicked".to_string()),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
    };
    info!(
        text_length_chars = extraction_result.text.len(),
        "Text extracted successfully"
    );

    let chunking_config: ChunkingConfig = match serde_json::from_value(job.chunking_config.clone())
    {
        Ok(config) => config,
        Err(e) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("transform-file", duration, "failed_config_parse");
            error!(error = %e, "Failed to parse chunking config");
            send_result(
                &ctx.nats_client,
                &job,
                Err(format!("Invalid chunking config: {}", e)),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
    };

    info!(
        chunk_size = chunking_config.chunk_size,
        strategy = ?chunking_config.strategy,
        overlap = chunking_config.chunk_overlap,
        "Chunking text"
    );

    let mut extraction_metadata = extraction_result
        .metadata
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = extraction_metadata.as_object_mut() {
        obj.insert(
            "source_file".to_string(),
            serde_json::json!(&job.source_file_key),
        );
    }

    let chunks_with_metadata = match ChunkingService::chunk_text(
        extraction_result.text.clone(),
        &chunking_config,
        Some(extraction_metadata),
        job.embedder_config.as_ref(), // Use embedder config from job for semantic chunking
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("transform-file", duration, "failed_chunking");
            error!(error = %e, "Chunking failed");
            send_result(
                &ctx.nats_client,
                &job,
                Err(e.to_string()),
                Some((duration * 1000.0) as i64),
            )
            .await?;
            return Ok(());
        }
    };

    info!(
        chunk_count = chunks_with_metadata.len(),
        "Text chunked successfully"
    );

    if chunks_with_metadata.is_empty() {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("transform-file", duration, "failed_empty_chunks");
        error!(
            text_length = extraction_result.text.len(),
            "Chunking produced no chunks (text too small or invalid). Raw text length: {} chars",
            extraction_result.text.len()
        );
        send_result(
            &ctx.nats_client,
            &job,
            Err("Chunking produced no chunks - text may be too short or invalid".to_string()),
            Some((duration * 1000.0) as i64),
        )
        .await?;
        return Ok(());
    }

    let chunks_key = format!("chunks/{}.json", job.job_id);
    let full_chunks_key = format!("collections/{}/{}", job.collection_id, chunks_key);
    let chunks_json = serde_json::to_vec(&chunks_with_metadata)?;
    let chunks_size = chunks_json.len();

    info!(chunks_size_bytes = chunks_size, key = %full_chunks_key, "Uploading chunks");
    if let Err(e) = upload_document(
        &ctx.s3_client,
        DocumentUpload {
            collection_id: s3_bucket_name.clone(),
            name: full_chunks_key.clone(),
            content: chunks_json,
            mime_type: "application/json".to_string(),
        },
    )
    .await
    {
        let duration = start_time.elapsed().as_secs_f64();
        record_worker_job("transform-file", duration, "failed_upload");
        error!(error = %e, "Failed to upload chunks");
        return Err(anyhow::anyhow!("Failed to upload chunks: {}", e));
    }

    let duration = start_time.elapsed().as_secs_f64();
    record_worker_job("transform-file", duration, "success");
    info!(duration_secs = duration, "Job completed successfully");
    send_result(
        &ctx.nats_client,
        &job,
        Ok((chunks_key, chunks_with_metadata.len())),
        Some((duration * 1000.0) as i64),
    )
    .await?;

    Ok(())
}

async fn send_result(
    nats: &async_nats::Client,
    job: &CollectionTransformJob,
    result: Result<(String, usize), String>,
    processing_duration_ms: Option<i64>,
) -> Result<()> {
    let (chunks_key, count, status, error) = match result {
        Ok((key, count)) => (key, count, "success".to_string(), None),
        Err(e) => ("".to_string(), 0, "failed".to_string(), Some(e)),
    };

    let result_msg = CollectionTransformResult {
        job_id: job.job_id,
        collection_transform_id: job.collection_transform_id,
        owner_id: job.owner_id.clone(),
        source_file_key: job.source_file_key.clone(),
        bucket: job.bucket.clone(),
        chunks_file_key: chunks_key,
        chunk_count: count,
        status,
        error,
        processing_duration_ms,
    };

    let subject = semantic_explorer_core::status::collection_status_subject(
        &job.owner_id,
        job.collection_id,
        job.collection_transform_id,
    );
    let payload = serde_json::to_vec(&result_msg)?;
    nats.publish(subject, payload.into()).await?;
    Ok(())
}
