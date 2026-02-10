//! Pending batches storage for resilient batch publishing
//!
//! When a batch fails to publish to NATS after retries, it's stored here for later recovery.
//! The reconciliation job periodically checks and retries these pending batches.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Postgres};
use std::time::Instant;
use tracing::instrument;
use utoipa::ToSchema;

use semantic_explorer_core::observability::record_database_query;

const INSERT_PENDING_BATCH_QUERY: &str = r#"
    INSERT INTO pending_batches (
        batch_type,
        dataset_transform_id,
        embedded_dataset_id,
        collection_transform_id,
        batch_key,
        s3_bucket,
        job_payload,
        next_retry_at,
        status,
        created_at,
        updated_at
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', NOW(), NOW())
    ON CONFLICT (batch_type, COALESCE(dataset_transform_id, 0), COALESCE(collection_transform_id, 0), batch_key) 
    WHERE status = 'pending'
    DO UPDATE SET
        retry_count = pending_batches.retry_count + 1,
        next_retry_at = $8,
        updated_at = NOW()
    RETURNING *
"#;

const GET_PENDING_BATCHES_FOR_RETRY_QUERY: &str = r#"
    SELECT *
    FROM pending_batches
    WHERE status = 'pending'
      AND next_retry_at <= NOW()
      AND retry_count < max_retries
    ORDER BY next_retry_at ASC
    LIMIT $1
    FOR UPDATE SKIP LOCKED
"#;

const MARK_BATCH_PUBLISHED_QUERY: &str = r#"
    UPDATE pending_batches
    SET status = 'published',
        updated_at = NOW()
    WHERE id = $1
    RETURNING *
"#;

const INCREMENT_RETRY_QUERY: &str = r#"
    UPDATE pending_batches
    SET retry_count = retry_count + 1,
        last_error = $2,
        next_retry_at = NOW() + ($3 * INTERVAL '1 second'),
        updated_at = NOW()
    WHERE id = $1
    RETURNING *
"#;

const MARK_BATCH_FAILED_QUERY: &str = r#"
    UPDATE pending_batches
    SET status = 'failed',
        last_error = $2,
        updated_at = NOW()
    WHERE id = $1
    RETURNING *
"#;

const CLEANUP_OLD_PUBLISHED_QUERY: &str = r#"
    DELETE FROM pending_batches
    WHERE status IN ('published', 'expired')
      AND updated_at < NOW() - INTERVAL '7 days'
    RETURNING id
"#;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct PendingBatch {
    pub id: i32,
    pub batch_type: String,
    pub dataset_transform_id: Option<i32>,
    pub embedded_dataset_id: Option<i32>,
    pub collection_transform_id: Option<i32>,
    pub batch_key: String,
    pub s3_bucket: String,
    pub job_payload: serde_json::Value,
    pub retry_count: i32,
    pub max_retries: i32,
    pub last_error: Option<String>,
    #[schema(value_type = String, format = DateTime)]
    pub next_retry_at: DateTime<Utc>,
    pub status: String,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct CreatePendingBatch {
    pub batch_type: String,
    pub dataset_transform_id: Option<i32>,
    pub embedded_dataset_id: Option<i32>,
    pub collection_transform_id: Option<i32>,
    pub batch_key: String,
    pub s3_bucket: String,
    pub job_payload: serde_json::Value,
}

/// Insert a pending batch for retry later
#[instrument(skip(pool, req), fields(batch_key = %req.batch_key))]
pub async fn insert_pending_batch(
    pool: &Pool<Postgres>,
    req: CreatePendingBatch,
) -> Result<PendingBatch> {
    let start = Instant::now();
    // Calculate next retry time with exponential backoff (starts at 30s)
    let next_retry_at = Utc::now() + chrono::Duration::seconds(30);

    let result = sqlx::query_as::<_, PendingBatch>(INSERT_PENDING_BATCH_QUERY)
        .bind(&req.batch_type)
        .bind(req.dataset_transform_id)
        .bind(req.embedded_dataset_id)
        .bind(req.collection_transform_id)
        .bind(&req.batch_key)
        .bind(&req.s3_bucket)
        .bind(&req.job_payload)
        .bind(next_retry_at)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    record_database_query("INSERT", "pending_batches", duration, result.is_ok());

    let batch = result.context("Failed to insert pending batch")?;
    Ok(batch)
}

/// Get pending batches ready for retry
#[instrument(skip(pool))]
pub async fn get_pending_batches_for_retry(
    pool: &Pool<Postgres>,
    limit: i64,
) -> Result<Vec<PendingBatch>> {
    let batches = sqlx::query_as::<_, PendingBatch>(GET_PENDING_BATCHES_FOR_RETRY_QUERY)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("Failed to get pending batches for retry")?;

    Ok(batches)
}

/// Mark a pending batch as successfully published
#[instrument(skip(pool))]
pub async fn mark_batch_published(pool: &Pool<Postgres>, batch_id: i32) -> Result<PendingBatch> {
    let batch = sqlx::query_as::<_, PendingBatch>(MARK_BATCH_PUBLISHED_QUERY)
        .bind(batch_id)
        .fetch_one(pool)
        .await
        .context("Failed to mark batch as published")?;

    Ok(batch)
}

/// Increment retry count and update next retry time with exponential backoff
#[instrument(skip(pool))]
pub async fn increment_retry(
    pool: &Pool<Postgres>,
    batch_id: i32,
    error: &str,
    current_retry_count: i32,
) -> Result<PendingBatch> {
    // Exponential backoff: 30s, 60s, 120s, 240s, ... up to 1 hour max
    let backoff_seconds = std::cmp::min(30 * 2i64.pow(current_retry_count as u32), 3600);

    let batch = sqlx::query_as::<_, PendingBatch>(INCREMENT_RETRY_QUERY)
        .bind(batch_id)
        .bind(error)
        .bind(backoff_seconds as i32)
        .fetch_one(pool)
        .await
        .context("Failed to increment retry count")?;

    Ok(batch)
}

/// Mark a pending batch as permanently failed (exceeded max retries)
#[instrument(skip(pool))]
pub async fn mark_batch_failed(
    pool: &Pool<Postgres>,
    batch_id: i32,
    error: &str,
) -> Result<PendingBatch> {
    let batch = sqlx::query_as::<_, PendingBatch>(MARK_BATCH_FAILED_QUERY)
        .bind(batch_id)
        .bind(error)
        .fetch_one(pool)
        .await
        .context("Failed to mark batch as failed")?;

    Ok(batch)
}

/// Cleanup old published/expired batches (housekeeping)
#[instrument(skip(pool))]
pub async fn cleanup_old_batches(pool: &Pool<Postgres>) -> Result<usize> {
    let deleted = sqlx::query_scalar::<_, i32>(CLEANUP_OLD_PUBLISHED_QUERY)
        .fetch_all(pool)
        .await
        .context("Failed to cleanup old batches")?;

    Ok(deleted.len())
}

/// Get orphaned pending batches older than the specified hours (#7)
/// These are batches that have been pending for too long and likely won't be processed
#[instrument(skip(pool))]
pub async fn get_orphaned_pending_batches(
    pool: &Pool<Postgres>,
    older_than_hours: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<PendingBatch>> {
    let batches = sqlx::query_as::<_, PendingBatch>(
        r#"
        SELECT *
        FROM pending_batches
        WHERE status = 'pending'
          AND created_at < NOW() - ($1 * INTERVAL '1 hour')
        ORDER BY created_at ASC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(older_than_hours)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to get orphaned pending batches")?;

    Ok(batches)
}
