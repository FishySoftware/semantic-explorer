use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Postgres, Row, Transaction};
use tracing::instrument;
use utoipa::ToSchema;

/// Helper struct for paginated batch queries that include total_count via COUNT(*) OVER()
#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
struct BatchWithCount {
    pub id: i32,
    pub dataset_transform_id: i32,
    pub batch_key: String,
    pub processed_at: DateTime<Utc>,
    pub status: String,
    pub chunk_count: i32,
    pub error_message: Option<String>,
    pub processing_duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_count: i64,
}

impl BatchWithCount {
    fn into_parts(rows: Vec<Self>) -> (Vec<DatasetTransformBatch>, i64) {
        let total_count = rows.first().map_or(0, |r| r.total_count);
        let batches = rows
            .into_iter()
            .map(|r| DatasetTransformBatch {
                id: r.id,
                dataset_transform_id: r.dataset_transform_id,
                batch_key: r.batch_key,
                processed_at: r.processed_at,
                status: r.status,
                chunk_count: r.chunk_count,
                error_message: r.error_message,
                processing_duration_ms: r.processing_duration_ms,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();
        (batches, total_count)
    }
}

/// Validate and return a safe sort field for batch queries.
/// Falls back to "processed_at" for unrecognized inputs.
fn validate_batch_sort_field(sort_by: &str) -> &'static str {
    match sort_by {
        "batch_key" => "batch_key",
        "status" => "status",
        "chunk_count" => "chunk_count",
        "processing_duration_ms" => "processing_duration_ms",
        "processed_at" => "processed_at",
        "created_at" => "created_at",
        _ => "processed_at",
    }
}

/// Validate and return a safe sort direction.
/// Falls back to "DESC" for unrecognized inputs.
fn validate_batch_sort_direction(direction: &str) -> &'static str {
    match direction.to_lowercase().as_str() {
        "asc" => "ASC",
        "desc" => "DESC",
        _ => "DESC",
    }
}

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
    ON CONFLICT (dataset_transform_id, batch_key)
    DO UPDATE SET
        status = EXCLUDED.status,
        chunk_count = EXCLUDED.chunk_count,
        error_message = EXCLUDED.error_message,
        processing_duration_ms = EXCLUDED.processing_duration_ms,
        updated_at = CURRENT_TIMESTAMP
    RETURNING *
"#;

const GET_BATCH_QUERY: &str = r#"
    SELECT * FROM dataset_transform_batches WHERE id = $1
"#;

// Static sort query variants for list_batches_by_transform (plan caching)
const BTF_BATCH_KEY_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY batch_key ASC LIMIT $2 OFFSET $3";
const BTF_BATCH_KEY_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY batch_key DESC LIMIT $2 OFFSET $3";
const BTF_STATUS_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY status ASC LIMIT $2 OFFSET $3";
const BTF_STATUS_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY status DESC LIMIT $2 OFFSET $3";
const BTF_CHUNK_COUNT_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY chunk_count ASC LIMIT $2 OFFSET $3";
const BTF_CHUNK_COUNT_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY chunk_count DESC LIMIT $2 OFFSET $3";
const BTF_DURATION_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY processing_duration_ms ASC LIMIT $2 OFFSET $3";
const BTF_DURATION_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY processing_duration_ms DESC LIMIT $2 OFFSET $3";
const BTF_PROCESSED_AT_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY processed_at ASC LIMIT $2 OFFSET $3";
const BTF_PROCESSED_AT_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY processed_at DESC LIMIT $2 OFFSET $3";
const BTF_CREATED_AT_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY created_at ASC LIMIT $2 OFFSET $3";
const BTF_CREATED_AT_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3";

fn get_btf_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("batch_key", "ASC") => BTF_BATCH_KEY_ASC,
        ("batch_key", "DESC") => BTF_BATCH_KEY_DESC,
        ("status", "ASC") => BTF_STATUS_ASC,
        ("status", "DESC") => BTF_STATUS_DESC,
        ("chunk_count", "ASC") => BTF_CHUNK_COUNT_ASC,
        ("chunk_count", "DESC") => BTF_CHUNK_COUNT_DESC,
        ("processing_duration_ms", "ASC") => BTF_DURATION_ASC,
        ("processing_duration_ms", "DESC") => BTF_DURATION_DESC,
        ("processed_at", "ASC") => BTF_PROCESSED_AT_ASC,
        ("created_at", "ASC") => BTF_CREATED_AT_ASC,
        ("created_at", "DESC") => BTF_CREATED_AT_DESC,
        _ => BTF_PROCESSED_AT_DESC, // default
    }
}

// Static sort query variants for list_batches_by_status (plan caching)
const BTS_BATCH_KEY_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY batch_key ASC LIMIT $3 OFFSET $4";
const BTS_BATCH_KEY_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY batch_key DESC LIMIT $3 OFFSET $4";
const BTS_STATUS_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY status ASC LIMIT $3 OFFSET $4";
const BTS_STATUS_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY status DESC LIMIT $3 OFFSET $4";
const BTS_CHUNK_COUNT_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY chunk_count ASC LIMIT $3 OFFSET $4";
const BTS_CHUNK_COUNT_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY chunk_count DESC LIMIT $3 OFFSET $4";
const BTS_DURATION_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY processing_duration_ms ASC LIMIT $3 OFFSET $4";
const BTS_DURATION_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY processing_duration_ms DESC LIMIT $3 OFFSET $4";
const BTS_PROCESSED_AT_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY processed_at ASC LIMIT $3 OFFSET $4";
const BTS_PROCESSED_AT_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY processed_at DESC LIMIT $3 OFFSET $4";
const BTS_CREATED_AT_ASC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY created_at ASC LIMIT $3 OFFSET $4";
const BTS_CREATED_AT_DESC: &str = "SELECT *, COUNT(*) OVER() AS total_count FROM dataset_transform_batches WHERE dataset_transform_id = $1 AND status = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4";

fn get_bts_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("batch_key", "ASC") => BTS_BATCH_KEY_ASC,
        ("batch_key", "DESC") => BTS_BATCH_KEY_DESC,
        ("status", "ASC") => BTS_STATUS_ASC,
        ("status", "DESC") => BTS_STATUS_DESC,
        ("chunk_count", "ASC") => BTS_CHUNK_COUNT_ASC,
        ("chunk_count", "DESC") => BTS_CHUNK_COUNT_DESC,
        ("processing_duration_ms", "ASC") => BTS_DURATION_ASC,
        ("processing_duration_ms", "DESC") => BTS_DURATION_DESC,
        ("processed_at", "ASC") => BTS_PROCESSED_AT_ASC,
        ("created_at", "ASC") => BTS_CREATED_AT_ASC,
        ("created_at", "DESC") => BTS_CREATED_AT_DESC,
        _ => BTS_PROCESSED_AT_DESC, // default
    }
}

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

const RESET_FAILED_BATCHES_QUERY: &str = r#"
    UPDATE dataset_transform_batches
    SET status = 'pending',
        error_message = NULL,
        processing_duration_ms = NULL,
        updated_at = CURRENT_TIMESTAMP
    WHERE dataset_transform_id = $1 AND status = 'failed'
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
    sort_by: &str,
    sort_direction: &str,
) -> Result<(Vec<DatasetTransformBatch>, i64), sqlx::Error> {
    let sort_field = validate_batch_sort_field(sort_by);
    let sort_dir = validate_batch_sort_direction(sort_direction);

    // Get paginated results with total count via COUNT(*) OVER()
    let query = get_btf_query(sort_field, sort_dir);
    let rows = sqlx::query_as::<_, BatchWithCount>(query)
        .bind(dataset_transform_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(BatchWithCount::into_parts(rows))
}

#[instrument(skip(pool), err)]
pub async fn list_batches_by_status(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
    status: &str,
    limit: i64,
    offset: i64,
    sort_by: &str,
    sort_direction: &str,
) -> Result<(Vec<DatasetTransformBatch>, i64), sqlx::Error> {
    let sort_field = validate_batch_sort_field(sort_by);
    let sort_dir = validate_batch_sort_direction(sort_direction);

    // Get paginated results with total count via COUNT(*) OVER()
    let query = get_bts_query(sort_field, sort_dir);
    let rows = sqlx::query_as::<_, BatchWithCount>(query)
        .bind(dataset_transform_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(BatchWithCount::into_parts(rows))
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

/// Reset all failed batches to "pending" for retry.
/// Returns the reset batches so they can be re-published.
pub async fn reset_failed_batches(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
) -> Result<Vec<DatasetTransformBatch>, sqlx::Error> {
    let batches = sqlx::query_as::<_, DatasetTransformBatch>(RESET_FAILED_BATCHES_QUERY)
        .bind(dataset_transform_id)
        .fetch_all(pool)
        .await?;
    Ok(batches)
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
