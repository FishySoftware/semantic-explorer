use anyhow::Result;
use semantic_explorer_core::jobs::{CollectionTransformJob, CollectionTransformResult};
use semantic_explorer_core::observability::record_worker_job;
use semantic_explorer_core::storage::{DocumentUpload, get_file, upload_document};
use std::time::Instant;
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
    info!("Processing file job");

    info!(bucket = %job.bucket, "Downloading file");
    let file_content = match get_file(&ctx.s3_client, &job.bucket, &job.source_file_key).await {
        Ok(content) => content,
        Err(e) => {
            let duration = start_time.elapsed().as_secs_f64();
            record_worker_job("transform-file", duration, "failed_download");
            error!(error = %e, "Failed to download file");
            send_result(
                &ctx.nats_client,
                &job,
                Err(format!("Download failed: {}", e)),
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

    let chunks_with_metadata = match ChunkingService::chunk_text(
        extraction_result.text,
        &chunking_config,
        extraction_result.metadata,
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

    let chunks_key = format!("chunks/{}.json", job.job_id);
    let chunks_json = serde_json::to_vec(&chunks_with_metadata)?;
    let chunks_size = chunks_json.len();

    info!(chunks_size_bytes = chunks_size, "Uploading chunks");
    if let Err(e) = upload_document(
        &ctx.s3_client,
        DocumentUpload {
            collection_id: job.bucket.clone(),
            name: chunks_key.clone(),
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

    info!("Chunks uploaded successfully");

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
        source_file_key: job.source_file_key.clone(),
        bucket: job.bucket.clone(),
        chunks_file_key: chunks_key,
        chunk_count: count,
        status,
        error,
        processing_duration_ms,
    };

    let payload = serde_json::to_vec(&result_msg)?;
    nats.publish("worker.result.file".to_string(), payload.into())
        .await?;
    Ok(())
}
