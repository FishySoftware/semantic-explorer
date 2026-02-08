use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Postgres, Row, Transaction};
use tracing::instrument;
use utoipa::ToSchema;

// SQL Query Constants
const CREATE_BATCH_QUERY: &str = r#"
    INSERT INTO dataset_transform_batches (
        dataset_transform_id,
        batch_key,
        status,
        chunk_count,
        error_message,
        processing_duration_ms,
        created_at,
        updated_at
    )
    VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    RETURNING *
"#;

const GET_BATCH_QUERY: &str = r#"
    SELECT * FROM dataset_transform_batches WHERE id = $1
"#;

const COUNT_BATCHES_BY_TRANSFORM_QUERY: &str = r#"
    SELECT COUNT(*) FROM dataset_transform_batches WHERE dataset_transform_id = $1
"#;

const LIST_BATCHES_BY_TRANSFORM_QUERY: &str = r#"
    SELECT * FROM dataset_transform_batches
    WHERE dataset_transform_id = $1
    ORDER BY processed_at DESC
    LIMIT $2 OFFSET $3
"#;

const COUNT_BATCHES_BY_TRANSFORM_AND_STATUS_QUERY: &str = r#"
    SELECT COUNT(*) FROM dataset_transform_batches
    WHERE dataset_transform_id = $1 AND status = $2
"#;

const LIST_BATCHES_BY_STATUS_QUERY: &str = r#"
    SELECT * FROM dataset_transform_batches
    WHERE dataset_transform_id = $1 AND status = $2
    ORDER BY processed_at DESC
    LIMIT $3 OFFSET $4
"#;

/// Query for detecting stuck batches across ALL transforms (#18)
const LIST_STUCK_BATCHES_QUERY: &str = r#"
    SELECT * FROM dataset_transform_batches
    WHERE status = $1
    AND created_at < NOW() - ($2 || ' hours')::INTERVAL
    ORDER BY created_at ASC
    LIMIT $3
"#;

const UPDATE_BATCH_STATUS_QUERY: &str = r#"
    UPDATE dataset_transform_batches
    SET status = $3,
        error_message = $4,
        processing_duration_ms = $5,
        chunk_count = $6,
        updated_at = CURRENT_TIMESTAMP
    WHERE dataset_transform_id = $1 AND batch_key = $2
    RETURNING *
"#;

const GET_BATCH_STATS_QUERY: &str = r#"
    SELECT
        COUNT(*) as total_batches,
        SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as successful_batches,
        SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed_batches,
        SUM(CASE WHEN status = 'pending' OR status = 'processing' THEN 1 ELSE 0 END) as processing_batches,
        SUM(chunk_count) as total_chunks,
        AVG(processing_duration_ms) as avg_duration_ms,
        MAX(processed_at) as last_processed_at
    FROM dataset_transform_batches
    WHERE dataset_transform_id = $1
"#;

#[derive(Serialize, Deserialize, Debug, Clone, FromRow, ToSchema)]
pub struct DatasetTransformBatch {
    pub id: i32,
    pub dataset_transform_id: i32,
    pub batch_key: String,
    #[schema(value_type = String, format = DateTime)]
    pub processed_at: DateTime<Utc>,
    pub status: String,
    pub chunk_count: i32,
    pub error_message: Option<String>,
    pub processing_duration_ms: Option<i64>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct CreateBatchRequest {
    pub dataset_transform_id: i32,
    pub batch_key: String,
    pub status: String,
    pub chunk_count: i32,
    pub error_message: Option<String>,
    pub processing_duration_ms: Option<i64>,
}

#[instrument(skip(pool), err)]
pub async fn get_batch(
    pool: &Pool<Postgres>,
    batch_id: i32,
) -> Result<Option<DatasetTransformBatch>, sqlx::Error> {
    let batch = sqlx::query_as::<_, DatasetTransformBatch>(GET_BATCH_QUERY)
        .bind(batch_id)
        .fetch_optional(pool)
        .await?;

    Ok(batch)
}

#[instrument(skip(pool), err)]
pub async fn list_batches_by_transform(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DatasetTransformBatch>, i64), sqlx::Error> {
    // Get total count
    let count_result = sqlx::query_scalar::<_, i64>(COUNT_BATCHES_BY_TRANSFORM_QUERY)
        .bind(dataset_transform_id)
        .fetch_one(pool)
        .await?;

    // Get paginated results sorted by processed_at DESC
    let batches = sqlx::query_as::<_, DatasetTransformBatch>(LIST_BATCHES_BY_TRANSFORM_QUERY)
        .bind(dataset_transform_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok((batches, count_result))
}

#[instrument(skip(pool), err)]
pub async fn list_batches_by_status(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
    status: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DatasetTransformBatch>, i64), sqlx::Error> {
    // Get total count
    let count_result = sqlx::query_scalar::<_, i64>(COUNT_BATCHES_BY_TRANSFORM_AND_STATUS_QUERY)
        .bind(dataset_transform_id)
        .bind(status)
        .fetch_one(pool)
        .await?;

    // Get paginated results
    let batches = sqlx::query_as::<_, DatasetTransformBatch>(LIST_BATCHES_BY_STATUS_QUERY)
        .bind(dataset_transform_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok((batches, count_result))
}

#[instrument(skip(pool), err)]
pub async fn get_batch_stats(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
) -> Result<BatchStats, sqlx::Error> {
    let result = sqlx::query(GET_BATCH_STATS_QUERY)
        .bind(dataset_transform_id)
        .fetch_one(pool)
        .await?;

    Ok(BatchStats {
        total_batches: result.get::<i64, _>("total_batches"),
        successful_batches: result
            .get::<Option<i64>, _>("successful_batches")
            .unwrap_or(0),
        failed_batches: result.get::<Option<i64>, _>("failed_batches").unwrap_or(0),
        processing_batches: result
            .get::<Option<i64>, _>("processing_batches")
            .unwrap_or(0),
        total_chunks: result.get::<Option<i64>, _>("total_chunks").unwrap_or(0),
        avg_duration_ms: result.get::<Option<f64>, _>("avg_duration_ms"),
        last_processed_at: result.get::<Option<DateTime<Utc>>, _>("last_processed_at"),
    })
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct BatchStats {
    pub total_batches: i64,
    pub successful_batches: i64,
    pub failed_batches: i64,
    pub processing_batches: i64,
    pub total_chunks: i64,
    pub avg_duration_ms: Option<f64>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_processed_at: Option<DateTime<Utc>>,
}

#[instrument(skip(tx), err)]
pub async fn create_batch_tx(
    tx: &mut Transaction<'_, Postgres>,
    req: CreateBatchRequest,
) -> Result<DatasetTransformBatch, sqlx::Error> {
    let batch = sqlx::query_as::<_, DatasetTransformBatch>(CREATE_BATCH_QUERY)
        .bind(req.dataset_transform_id)
        .bind(&req.batch_key)
        .bind(&req.status)
        .bind(req.chunk_count)
        .bind(&req.error_message)
        .bind(req.processing_duration_ms)
        .fetch_one(&mut **tx)
        .await?;

    Ok(batch)
}

#[instrument(skip(tx), err)]
pub async fn update_batch_status_tx(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    batch_key: &str,
    status: &str,
    error_message: Option<&str>,
    processing_duration_ms: Option<i64>,
    chunk_count: i32,
) -> Result<DatasetTransformBatch, sqlx::Error> {
    let batch = sqlx::query_as::<_, DatasetTransformBatch>(UPDATE_BATCH_STATUS_QUERY)
        .bind(dataset_transform_id)
        .bind(batch_key)
        .bind(status)
        .bind(error_message)
        .bind(processing_duration_ms)
        .bind(chunk_count)
        .fetch_one(&mut **tx)
        .await?;

    Ok(batch)
}

/// List batches that appear stuck in a given status for longer than threshold_hours (#18)
pub async fn list_stuck_batches(
    pool: &Pool<Postgres>,
    status: &str,
    threshold_hours: i64,
    limit: i64,
) -> Result<Vec<DatasetTransformBatch>, sqlx::Error> {
    let batches = sqlx::query_as::<_, DatasetTransformBatch>(LIST_STUCK_BATCHES_QUERY)
        .bind(status)
        .bind(threshold_hours.to_string())
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(batches)
}
