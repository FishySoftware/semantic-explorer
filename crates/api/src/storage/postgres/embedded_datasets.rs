use anyhow::Result;
use semantic_explorer_core::owner_info::OwnerInfo;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{FromRow, Pool, Postgres, Transaction};

use crate::embedded_datasets::{
    EmbeddedDataset, EmbeddedDatasetProcessedBatch, EmbeddedDatasetStats,
    EmbeddedDatasetWithDetails,
};

#[derive(FromRow)]
pub struct EmbeddedDatasetInfo {
    pub collection_name: String,
    pub embedder_id: i32,
}

/// Helper struct for paginated queries that include total_count via COUNT(*) OVER()
#[derive(FromRow)]
struct EmbeddedDatasetWithCount {
    pub embedded_dataset_id: i32,
    pub title: String,
    pub dataset_transform_id: i32,
    pub source_dataset_id: i32,
    pub embedder_id: i32,
    pub owner_id: String,
    pub owner_display_name: String,
    pub collection_name: String,
    pub dimensions: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_processed_at: Option<DateTime<Utc>>,
    pub last_processed_item_id: Option<i32>,
    pub source_dataset_version: Option<DateTime<Utc>>,
    pub total_count: i64,
}

impl EmbeddedDatasetWithCount {
    fn into_parts(rows: Vec<Self>) -> (Vec<EmbeddedDataset>, i64) {
        let total_count = rows.first().map_or(0, |r| r.total_count);
        let datasets = rows
            .into_iter()
            .map(|r| EmbeddedDataset {
                embedded_dataset_id: r.embedded_dataset_id,
                title: r.title,
                dataset_transform_id: r.dataset_transform_id,
                source_dataset_id: r.source_dataset_id,
                embedder_id: r.embedder_id,
                owner_id: r.owner_id,
                owner_display_name: r.owner_display_name,
                collection_name: r.collection_name,
                dimensions: r.dimensions,
                created_at: r.created_at,
                updated_at: r.updated_at,
                last_processed_at: r.last_processed_at,
                last_processed_item_id: r.last_processed_item_id,
                source_dataset_version: r.source_dataset_version,
            })
            .collect();
        (datasets, total_count)
    }
}

const COUNT_EMBEDDED_DATASETS_BY_EMBEDDER_QUERY: &str = r#"
    SELECT COUNT(*) FROM embedded_datasets
    WHERE embedder_id = $1 AND owner_id = $2
"#;

const GET_EMBEDDED_DATASET_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
    FROM embedded_datasets
    WHERE embedded_dataset_id = $1 AND owner_id = $2
"#;

/// Get embedded dataset by ID without owner check (privileged, for internal use only)
const GET_EMBEDDED_DATASET_BY_ID_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
    FROM embedded_datasets
    WHERE embedded_dataset_id = $1
"#;

const GET_EMBEDDED_DATASETS_PAGINATED_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version,
           COUNT(*) OVER() AS total_count
    FROM embedded_datasets
    WHERE owner_id = $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const GET_EMBEDDED_DATASETS_WITH_SEARCH_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version,
           COUNT(*) OVER() AS total_count
    FROM embedded_datasets
    WHERE owner_id = $1 AND title ILIKE $2
    ORDER BY created_at DESC
    LIMIT $3 OFFSET $4
"#;

const GET_EMBEDDED_DATASETS_FOR_DATASET_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
    FROM embedded_datasets
    WHERE source_dataset_id = $1 AND owner_id = $2
    ORDER BY created_at DESC
    LIMIT $3 OFFSET $4
"#;

const GET_EMBEDDED_DATASETS_FOR_TRANSFORM_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
    FROM embedded_datasets
    WHERE dataset_transform_id = $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const GET_EMBEDDED_DATASET_WITH_DETAILS_QUERY: &str = r#"
    SELECT
        ed.embedded_dataset_id,
        ed.title,
        ed.dataset_transform_id,
        ed.source_dataset_id,
        COALESCE(d.title, 'N/A') as source_dataset_title,
        ed.embedder_id,
        COALESCE(e.name, 'N/A') as embedder_name,
        ed.owner_id,
        ed.owner_display_name,
        ed.collection_name,
        COALESCE(ed.dimensions, e.dimensions) as dimensions,
        (ed.dataset_transform_id = 0 AND ed.source_dataset_id = 0 AND ed.embedder_id = 0) as is_standalone,
        ct.collection_id,
        c.title as collection_title,
        ed.created_at,
        ed.updated_at
    FROM embedded_datasets ed
    LEFT JOIN datasets d ON d.dataset_id = ed.source_dataset_id AND ed.source_dataset_id != 0
    LEFT JOIN embedders e ON e.embedder_id = ed.embedder_id AND ed.embedder_id != 0
    LEFT JOIN collection_transforms ct ON ct.dataset_id = d.dataset_id
    LEFT JOIN collections c ON c.collection_id = ct.collection_id
    WHERE ed.owner_id = $1 AND ed.embedded_dataset_id = $2
"#;

const CREATE_EMBEDDED_DATASET_QUERY: &str = r#"
    INSERT INTO embedded_datasets (title, dataset_transform_id, source_dataset_id, embedder_id, owner_id, owner_display_name, collection_name, dimensions)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
"#;

const UPDATE_EMBEDDED_DATASET_COLLECTION_NAME_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET collection_name = $2,
        updated_at = NOW()
    WHERE embedded_dataset_id = $1
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
"#;

const UPDATE_EMBEDDED_DATASET_TITLE_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET title = $2,
        updated_at = NOW()
    WHERE embedded_dataset_id = $1 AND owner_id = $3
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
"#;

const DELETE_EMBEDDED_DATASET_QUERY: &str = r#"
    DELETE FROM embedded_datasets
    WHERE embedded_dataset_id = $1 AND owner_id = $2
"#;

/// Delete by ID without owner check (privileged, for internal use only)
const DELETE_EMBEDDED_DATASET_BY_ID_QUERY: &str = r#"
    DELETE FROM embedded_datasets
    WHERE embedded_dataset_id = $1
"#;

const GET_EMBEDDED_DATASET_STATS_QUERY: &str = r#"
    SELECT
        $1::INTEGER as embedded_dataset_id,
        COALESCE(COUNT(tpf.id), 0)::BIGINT as total_batches_processed,
        COALESCE(COUNT(tpf.id) FILTER (WHERE tpf.process_status = 'completed'), 0)::BIGINT as successful_batches,
        COALESCE(COUNT(tpf.id) FILTER (WHERE tpf.process_status = 'failed'), 0)::BIGINT as failed_batches,
        COALESCE(COUNT(tpf.id) FILTER (WHERE tpf.process_status = 'processing'), 0)::BIGINT as processing_batches,
        COALESCE(SUM(tpf.item_count) FILTER (WHERE tpf.process_status = 'completed'), 0)::BIGINT as total_chunks_embedded,
        COALESCE(SUM(tpf.item_count) FILTER (WHERE tpf.process_status = 'failed'), 0)::BIGINT as total_chunks_failed,
        COALESCE(SUM(tpf.item_count) FILTER (WHERE tpf.process_status = 'processing'), 0)::BIGINT as total_chunks_processing,
        MAX(tpf.processed_at) as last_run_at,
        MIN(tpf.processed_at) FILTER (WHERE tpf.process_status = 'processing') as first_processing_at,
        AVG(tpf.processing_duration_ms) FILTER (WHERE tpf.process_status = 'completed')::BIGINT as avg_processing_duration_ms
    FROM embedded_datasets ed
    LEFT JOIN transform_processed_files tpf ON tpf.transform_type = 'dataset' AND tpf.transform_id = ed.embedded_dataset_id
    WHERE ed.embedded_dataset_id = $1 AND ed.owner_id = $2
"#;

const GET_PROCESSED_BATCHES_QUERY: &str = r#"
    SELECT
        id,
        transform_id as embedded_dataset_id,
        file_key,
        processed_at,
        item_count,
        process_status,
        process_error,
        processing_duration_ms
    FROM transform_processed_files
    WHERE transform_type = 'dataset' AND transform_id = $1
    ORDER BY processed_at DESC
    LIMIT $2 OFFSET $3
"#;

const RECORD_PROCESSED_BATCH_QUERY: &str = r#"
    INSERT INTO transform_processed_files
        (transform_type, transform_id, file_key, item_count, process_status, process_error, processing_duration_ms)
    VALUES ('dataset', $1, $2, $3, $4, $5, $6)
    ON CONFLICT (transform_type, transform_id, file_key)
    DO UPDATE SET
        item_count = EXCLUDED.item_count,
        process_status = EXCLUDED.process_status,
        process_error = EXCLUDED.process_error,
        processing_duration_ms = EXCLUDED.processing_duration_ms,
        processed_at = NOW()
"#;

const GET_BATCH_PREVIOUS_STATUS_QUERY: &str = r#"
    SELECT process_status
    FROM transform_processed_files
    WHERE transform_type = 'dataset'
      AND transform_id = $1
      AND file_key = $2
"#;

/// Delete processed-file records for specific batch keys across all embedded datasets
/// of a given dataset transform. This allows the scanner to rediscover them for retry.
const DELETE_PROCESSED_BATCHES_FOR_RETRY_QUERY: &str = r#"
    DELETE FROM transform_processed_files
    WHERE transform_type = 'dataset'
      AND transform_id = ANY(
          SELECT embedded_dataset_id FROM embedded_datasets
          WHERE dataset_transform_id = $1
      )
      AND file_key = ANY($2)
"#;

const GET_EMBEDDED_DATASET_INFO_QUERY: &str = r#"
    SELECT collection_name, embedder_id FROM embedded_datasets WHERE embedded_dataset_id = $1
"#;

const UPDATE_EMBEDDED_DATASET_LAST_PROCESSED_AT_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET last_processed_at = $2,
        last_processed_item_id = $3
    WHERE embedded_dataset_id = $1
"#;

/// Atomically acquire a scan lock on an embedded dataset.
/// Returns true (1 row updated) if the lock was acquired, false if another scanner holds it.
/// The lock expires after the specified timeout so stale locks don't block forever.
const ACQUIRE_SCAN_LOCK_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET scan_locked_at = NOW()
    WHERE embedded_dataset_id = $1
      AND (scan_locked_at IS NULL OR scan_locked_at < NOW() - $2::interval)
"#;

/// Release the scan lock after processing is complete.
const RELEASE_SCAN_LOCK_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET scan_locked_at = NULL
    WHERE embedded_dataset_id = $1
"#;

const UPDATE_SOURCE_DATASET_VERSION_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET source_dataset_version = $2,
        updated_at = NOW()
    WHERE embedded_dataset_id = $1
"#;

const GET_EMBEDDED_DATASET_WITH_DETAILS_BATCH: &str = r#"
        SELECT
            ed.embedded_dataset_id,
            ed.title,
            ed.dataset_transform_id,
            ed.source_dataset_id,
            COALESCE(d.title, 'N/A') as source_dataset_title,
            ed.embedder_id,
            COALESCE(e.name, 'N/A') as embedder_name,
            ed.owner_id,
            ed.owner_display_name,
            ed.collection_name,
            COALESCE(ed.dimensions, e.dimensions) as dimensions,
            (ed.dataset_transform_id = 0 AND ed.source_dataset_id = 0 AND ed.embedder_id = 0) as is_standalone,
            ct.collection_id,
            c.title as collection_title,
            ed.created_at,
            ed.updated_at
        FROM embedded_datasets ed
        LEFT JOIN datasets d ON d.dataset_id = ed.source_dataset_id AND ed.source_dataset_id != 0
        LEFT JOIN embedders e ON e.embedder_id = ed.embedder_id AND ed.embedder_id != 0
        LEFT JOIN collection_transforms ct ON ct.dataset_id = d.dataset_id
        LEFT JOIN collections c ON c.collection_id = ct.collection_id
        WHERE ed.owner_id = $1 AND ed.embedded_dataset_id = ANY($2)
        ORDER BY ed.embedded_dataset_id
        "#;

// Standalone embedded dataset queries
const CREATE_STANDALONE_EMBEDDED_DATASET_QUERY: &str = r#"
    INSERT INTO embedded_datasets (title, dataset_transform_id, source_dataset_id, embedder_id, owner_id, owner_display_name, collection_name, dimensions)
    VALUES ($1, 0, 0, 0, $2, $3, $4, $5)
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner_id, owner_display_name, collection_name, dimensions, created_at, updated_at, last_processed_at, last_processed_item_id, source_dataset_version
"#;

/// Count how many embedded datasets reference a given embedder.
#[tracing::instrument(name = "database.count_embedded_datasets_by_embedder", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub async fn count_by_embedder(
    pool: &Pool<Postgres>,
    user: &crate::auth::AuthenticatedUser,
    embedder_id: i32,
) -> Result<i64> {
    let result = sqlx::query_as::<_, (i64,)>(COUNT_EMBEDDED_DATASETS_BY_EMBEDDER_QUERY)
        .bind(embedder_id)
        .bind(user.as_owner())
        .fetch_one(pool)
        .await;

    Ok(result?.0)
}

pub async fn get_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDataset> {
    let result = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASET_QUERY)
        .bind(embedded_dataset_id)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let embedded_dataset = result?;

    Ok(embedded_dataset)
}

/// Get embedded dataset by ID without owner check (privileged, for internal use only)
/// Used by reconciliation job to verify datasets exist
pub async fn get_embedded_dataset_by_id(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDataset> {
    let embedded_dataset = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASET_BY_ID_QUERY)
        .bind(embedded_dataset_id)
        .fetch_one(pool)
        .await?;

    Ok(embedded_dataset)
}

pub async fn get_embedded_datasets_paginated(
    pool: &Pool<Postgres>,
    owner: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<EmbeddedDataset>, i64)> {
    let rows = sqlx::query_as::<_, EmbeddedDatasetWithCount>(GET_EMBEDDED_DATASETS_PAGINATED_QUERY)
        .bind(owner)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(EmbeddedDatasetWithCount::into_parts(rows))
}

pub async fn get_embedded_datasets_with_search(
    pool: &Pool<Postgres>,
    owner: &str,
    search_query: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<EmbeddedDataset>, i64)> {
    let search_pattern = format!("%{}%", search_query);

    let rows =
        sqlx::query_as::<_, EmbeddedDatasetWithCount>(GET_EMBEDDED_DATASETS_WITH_SEARCH_QUERY)
            .bind(owner)
            .bind(&search_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

    Ok(EmbeddedDatasetWithCount::into_parts(rows))
}

pub async fn get_embedded_datasets_for_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<EmbeddedDataset>> {
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_FOR_DATASET_QUERY)
            .bind(dataset_id)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

    Ok(embedded_datasets)
}

pub async fn get_embedded_datasets_for_transform(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<EmbeddedDataset>> {
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_FOR_TRANSFORM_QUERY)
            .bind(dataset_transform_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
    Ok(embedded_datasets)
}

pub async fn get_embedded_dataset_with_details(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDatasetWithDetails> {
    let embedded_dataset =
        sqlx::query_as::<_, EmbeddedDatasetWithDetails>(GET_EMBEDDED_DATASET_WITH_DETAILS_QUERY)
            .bind(owner)
            .bind(embedded_dataset_id)
            .fetch_one(pool)
            .await?;

    Ok(embedded_dataset)
}

/// Batch fetch embedded datasets with details (avoids N+1 queries)
pub async fn get_embedded_datasets_with_details_batch(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_ids: &[i32],
) -> Result<Vec<EmbeddedDatasetWithDetails>> {
    if embedded_dataset_ids.is_empty() {
        return Ok(Vec::new());
    }

    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDatasetWithDetails>(GET_EMBEDDED_DATASET_WITH_DETAILS_BATCH)
            .bind(owner)
            .bind(embedded_dataset_ids.to_vec())
            .fetch_all(pool)
            .await?;

    Ok(embedded_datasets)
}

pub async fn delete_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<()> {
    // Verify ownership first
    let _ = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASET_QUERY)
        .bind(embedded_dataset_id)
        .bind(owner)
        .fetch_one(pool)
        .await?;

    sqlx::query(DELETE_EMBEDDED_DATASET_QUERY)
        .bind(embedded_dataset_id)
        .bind(owner)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn update_embedded_dataset_title(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
    title: &str,
) -> Result<EmbeddedDataset> {
    let embedded_dataset =
        sqlx::query_as::<_, EmbeddedDataset>(UPDATE_EMBEDDED_DATASET_TITLE_QUERY)
            .bind(embedded_dataset_id)
            .bind(title)
            .bind(owner)
            .fetch_optional(pool)
            .await?;

    match embedded_dataset {
        Some(dataset) => Ok(dataset),
        None => Err(anyhow::anyhow!(
            "Embedded dataset not found or not owned by this user"
        )),
    }
}

pub async fn get_embedded_dataset_stats(
    pool: &Pool<Postgres>,
    owner_id: &str,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDatasetStats> {
    let result = sqlx::query_as::<_, EmbeddedDatasetStats>(GET_EMBEDDED_DATASET_STATS_QUERY)
        .bind(embedded_dataset_id)
        .bind(owner_id)
        .fetch_one(pool)
        .await;
    let stats = result?;
    Ok(stats)
}

pub async fn get_processed_batches(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<EmbeddedDatasetProcessedBatch>> {
    let batches = sqlx::query_as::<_, EmbeddedDatasetProcessedBatch>(GET_PROCESSED_BATCHES_QUERY)
        .bind(embedded_dataset_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(batches)
}

/// Get the previous status of a batch from transform_processed_files
/// Used to determine state transitions for atomic stats updates
pub async fn get_batch_previous_status(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    batch_file_key: &str,
) -> Option<String> {
    sqlx::query_scalar::<_, String>(GET_BATCH_PREVIOUS_STATUS_QUERY)
        .bind(embedded_dataset_id)
        .bind(batch_file_key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// Record a processed batch within a transaction (for atomic updates with stats)
pub async fn record_processed_batch_tx(
    tx: &mut Transaction<'_, Postgres>,
    embedded_dataset_id: i32,
    file_key: &str,
    item_count: i32,
    process_status: &str,
    process_error: Option<&str>,
    processing_duration_ms: Option<i64>,
) -> Result<()> {
    sqlx::query(RECORD_PROCESSED_BATCH_QUERY)
        .bind(embedded_dataset_id)
        .bind(file_key)
        .bind(item_count)
        .bind(process_status)
        .bind(process_error)
        .bind(processing_duration_ms)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Delete processed-file records for the given batch keys so the scanner
/// can rediscover them during a retry. Deletes across all embedded datasets
/// that belong to the specified dataset transform.
pub async fn delete_processed_batches_for_retry(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
    batch_keys: &[String],
) -> Result<u64> {
    let result = sqlx::query(DELETE_PROCESSED_BATCHES_FOR_RETRY_QUERY)
        .bind(dataset_transform_id)
        .bind(batch_keys)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Update the last_processed_at timestamp and last_processed_item_id to specific values.
/// Uses composite (timestamp, item_id) watermark for correct keyset pagination
/// when items share the same timestamp (common with batch inserts).
pub async fn update_embedded_dataset_last_processed_at_to(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    timestamp: DateTime<Utc>,
    last_processed_item_id: Option<i32>,
) -> Result<()> {
    sqlx::query(UPDATE_EMBEDDED_DATASET_LAST_PROCESSED_AT_QUERY)
        .bind(embedded_dataset_id)
        .bind(timestamp)
        .bind(last_processed_item_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Attempt to acquire a scan lock on an embedded dataset.
/// Returns true if the lock was acquired, false if another scanner holds it.
/// The lock auto-expires after `lock_timeout_secs` to prevent stale locks.
pub async fn try_acquire_scan_lock(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    lock_timeout_secs: u64,
) -> Result<bool> {
    let interval = format!("{} seconds", lock_timeout_secs);
    let result = sqlx::query(ACQUIRE_SCAN_LOCK_QUERY)
        .bind(embedded_dataset_id)
        .bind(&interval)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Release the scan lock on an embedded dataset after processing.
pub async fn release_scan_lock(pool: &Pool<Postgres>, embedded_dataset_id: i32) -> Result<()> {
    sqlx::query(RELEASE_SCAN_LOCK_QUERY)
        .bind(embedded_dataset_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Update the source dataset version tracker (#4)
/// Used to avoid unnecessary stats refresh when the source dataset hasn't changed
pub async fn update_source_dataset_version(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    version: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(UPDATE_SOURCE_DATASET_VERSION_QUERY)
        .bind(embedded_dataset_id)
        .bind(version)
        .execute(pool)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_embedded_dataset_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    title: &str,
    dataset_transform_id: i32,
    source_dataset_id: i32,
    embedder_id: i32,
    owner: &OwnerInfo,
    collection_name: &str,
    dimensions: Option<i32>,
) -> Result<EmbeddedDataset> {
    let mut embedded_dataset = sqlx::query_as::<_, EmbeddedDataset>(CREATE_EMBEDDED_DATASET_QUERY)
        .bind(title)
        .bind(dataset_transform_id)
        .bind(source_dataset_id)
        .bind(embedder_id)
        .bind(&owner.owner_id)
        .bind(&owner.owner_display_name)
        .bind(collection_name)
        .bind(dimensions)
        .fetch_one(&mut **tx)
        .await?;

    let actual_collection_name = EmbeddedDataset::generate_collection_name(
        embedded_dataset.embedded_dataset_id,
        &owner.owner_id,
    );
    embedded_dataset =
        sqlx::query_as::<_, EmbeddedDataset>(UPDATE_EMBEDDED_DATASET_COLLECTION_NAME_QUERY)
            .bind(embedded_dataset.embedded_dataset_id)
            .bind(&actual_collection_name)
            .fetch_one(&mut **tx)
            .await?;

    Ok(embedded_dataset)
}

pub async fn get_embedded_datasets_for_transform_in_transaction(
    executor: &mut sqlx::PgConnection,
    dataset_transform_id: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<EmbeddedDataset>> {
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_FOR_TRANSFORM_QUERY)
            .bind(dataset_transform_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(executor)
            .await?;
    Ok(embedded_datasets)
}

pub async fn delete_embedded_dataset_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    embedded_dataset_id: i32,
) -> Result<()> {
    sqlx::query(DELETE_EMBEDDED_DATASET_BY_ID_QUERY)
        .bind(embedded_dataset_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn get_embedded_dataset_info(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDatasetInfo> {
    let info = sqlx::query_as::<_, EmbeddedDatasetInfo>(GET_EMBEDDED_DATASET_INFO_QUERY)
        .bind(embedded_dataset_id)
        .fetch_one(pool)
        .await?;
    Ok(info)
}

/// Create a standalone embedded dataset (without transform/dataset/embedder)
/// These datasets are created with sentinel value 0 for all FK fields
/// and require dimensions to be specified upfront
pub async fn create_standalone_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &OwnerInfo,
    title: &str,
    dimensions: i32,
) -> Result<EmbeddedDataset> {
    let mut tx = pool.begin().await?;

    // Create with placeholder collection name, will update after we have the ID
    let mut embedded_dataset =
        sqlx::query_as::<_, EmbeddedDataset>(CREATE_STANDALONE_EMBEDDED_DATASET_QUERY)
            .bind(title)
            .bind(&owner.owner_id)
            .bind(&owner.owner_display_name)
            .bind("placeholder") // Will be updated below
            .bind(dimensions)
            .fetch_one(&mut *tx)
            .await?;

    // Update with actual collection name using the generated ID
    let actual_collection_name = EmbeddedDataset::generate_collection_name(
        embedded_dataset.embedded_dataset_id,
        &owner.owner_id,
    );
    embedded_dataset =
        sqlx::query_as::<_, EmbeddedDataset>(UPDATE_EMBEDDED_DATASET_COLLECTION_NAME_QUERY)
            .bind(embedded_dataset.embedded_dataset_id)
            .bind(&actual_collection_name)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(embedded_dataset)
}
