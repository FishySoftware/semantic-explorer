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

const GET_EMBEDDED_DATASET_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
    FROM embedded_datasets
    WHERE embedded_dataset_id = $1
"#;

const COUNT_EMBEDDED_DATASETS_QUERY: &str = r#"
    SELECT COUNT(*)
    FROM embedded_datasets
"#;

const COUNT_EMBEDDED_DATASETS_SEARCH_QUERY: &str = r#"
    SELECT COUNT(*)
    FROM embedded_datasets
    WHERE title ILIKE $1
"#;

const GET_EMBEDDED_DATASETS_PAGINATED_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
    FROM embedded_datasets
    ORDER BY created_at DESC
    LIMIT $1 OFFSET $2
"#;

const GET_EMBEDDED_DATASETS_WITH_SEARCH_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
    FROM embedded_datasets
    WHERE title ILIKE $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const GET_EMBEDDED_DATASETS_FOR_DATASET_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
    FROM embedded_datasets
    WHERE source_dataset_id = $1
    ORDER BY created_at DESC
"#;

const GET_EMBEDDED_DATASETS_FOR_TRANSFORM_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
    FROM embedded_datasets
    WHERE dataset_transform_id = $1
    ORDER BY created_at DESC
"#;

const GET_EMBEDDED_DATASET_WITH_DETAILS_QUERY: &str = r#"
    SELECT
        ed.embedded_dataset_id,
        ed.title,
        ed.dataset_transform_id,
        ed.source_dataset_id,
        d.title as source_dataset_title,
        ed.embedder_id,
        e.name as embedder_name,
        ed.owner_id,
        ed.owner_display_name,
        ed.collection_name,
        ed.created_at,
        ed.updated_at
    FROM embedded_datasets ed
    INNER JOIN datasets d ON d.dataset_id = ed.source_dataset_id
    INNER JOIN embedders e ON e.embedder_id = ed.embedder_id
    WHERE ed.owner_id = $1 AND ed.embedded_dataset_id = $2
"#;

const CREATE_EMBEDDED_DATASET_QUERY: &str = r#"
    INSERT INTO embedded_datasets (title, dataset_transform_id, source_dataset_id, embedder_id, owner_id, owner_display_name, collection_name)
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
"#;

const UPDATE_EMBEDDED_DATASET_COLLECTION_NAME_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET collection_name = $2,
        updated_at = NOW()
    WHERE embedded_dataset_id = $1
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
"#;

const UPDATE_EMBEDDED_DATASET_TITLE_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET title = $2,
        updated_at = NOW()
    WHERE embedded_dataset_id = $1 AND owner_id = $3
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner_id, owner_display_name, collection_name, created_at, updated_at, last_processed_at
"#;

const DELETE_EMBEDDED_DATASET_QUERY: &str = r#"
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
    WHERE ed.embedded_dataset_id = $1
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

const GET_EMBEDDED_DATASET_INFO_QUERY: &str = r#"
    SELECT collection_name, embedder_id FROM embedded_datasets WHERE embedded_dataset_id = $1
"#;

const UPDATE_EMBEDDED_DATASET_LAST_PROCESSED_AT_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET last_processed_at = $2
    WHERE embedded_dataset_id = $1
"#;

const GET_EMBEDDED_DATASET_WITH_DETAILS_BATCH: &str = r#"
        SELECT
            ed.embedded_dataset_id,
            ed.title,
            ed.dataset_transform_id,
            ed.source_dataset_id,
            d.title as source_dataset_title,
            ed.embedder_id,
            e.name as embedder_name,
            ed.owner_id,
            ed.owner_display_name,
            ed.collection_name,
            ed.created_at,
            ed.updated_at
        FROM embedded_datasets ed
        INNER JOIN datasets d ON d.dataset_id = ed.source_dataset_id
        INNER JOIN embedders e ON e.embedder_id = ed.embedder_id
        WHERE ed.owner_id = $1 AND ed.embedded_dataset_id = ANY($2)
        ORDER BY ed.embedded_dataset_id
        "#;

pub async fn get_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDataset> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let embedded_dataset = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASET_QUERY)
        .bind(embedded_dataset_id)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(embedded_dataset)
}

/// Get embedded dataset by ID without owner check (privileged, for internal use only)
/// Used by reconciliation job to verify datasets exist
pub async fn get_embedded_dataset_by_id(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDataset> {
    let embedded_dataset = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASET_QUERY)
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
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    // Get total count
    let count_result = sqlx::query_as::<_, (i64,)>(COUNT_EMBEDDED_DATASETS_QUERY)
        .fetch_one(&mut *tx)
        .await?;
    let total_count = count_result.0;

    // Get paginated results
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_PAGINATED_QUERY)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok((embedded_datasets, total_count))
}

pub async fn get_embedded_datasets_with_search(
    pool: &Pool<Postgres>,
    owner: &str,
    search_query: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<EmbeddedDataset>, i64)> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let search_pattern = format!("%{}%", search_query);

    // Get total count with search filter
    let count_result = sqlx::query_as::<_, (i64,)>(COUNT_EMBEDDED_DATASETS_SEARCH_QUERY)
        .bind(&search_pattern)
        .fetch_one(&mut *tx)
        .await?;
    let total_count = count_result.0;

    // Get paginated results with search filter
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_WITH_SEARCH_QUERY)
            .bind(&search_pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok((embedded_datasets, total_count))
}

pub async fn get_embedded_datasets_for_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
) -> Result<Vec<EmbeddedDataset>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_FOR_DATASET_QUERY)
            .bind(dataset_id)
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(embedded_datasets)
}

pub async fn get_embedded_datasets_for_transform(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
) -> Result<Vec<EmbeddedDataset>> {
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_FOR_TRANSFORM_QUERY)
            .bind(dataset_transform_id)
            .fetch_all(pool)
            .await?;
    Ok(embedded_datasets)
}

pub async fn get_embedded_dataset_with_details(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDatasetWithDetails> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let embedded_dataset =
        sqlx::query_as::<_, EmbeddedDatasetWithDetails>(GET_EMBEDDED_DATASET_WITH_DETAILS_QUERY)
            .bind(owner)
            .bind(embedded_dataset_id)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;
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

    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDatasetWithDetails>(GET_EMBEDDED_DATASET_WITH_DETAILS_BATCH)
            .bind(owner)
            .bind(embedded_dataset_ids.to_vec())
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(embedded_datasets)
}

pub async fn delete_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    // Verify ownership first
    let _ = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASET_QUERY)
        .bind(embedded_dataset_id)
        .fetch_one(&mut *tx)
        .await?;

    sqlx::query(DELETE_EMBEDDED_DATASET_QUERY)
        .bind(embedded_dataset_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn update_embedded_dataset_title(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
    title: &str,
) -> Result<EmbeddedDataset> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let embedded_dataset =
        sqlx::query_as::<_, EmbeddedDataset>(UPDATE_EMBEDDED_DATASET_TITLE_QUERY)
            .bind(embedded_dataset_id)
            .bind(title)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await?;

    match embedded_dataset {
        Some(dataset) => {
            tx.commit().await?;
            Ok(dataset)
        }
        None => Err(anyhow::anyhow!(
            "Embedded dataset not found or not owned by this user"
        )),
    }
}

pub async fn get_embedded_dataset_stats(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDatasetStats> {
    let stats = sqlx::query_as::<_, EmbeddedDatasetStats>(GET_EMBEDDED_DATASET_STATS_QUERY)
        .bind(embedded_dataset_id)
        .fetch_one(pool)
        .await?;
    Ok(stats)
}

pub async fn get_batch_embedded_dataset_stats(
    pool: &Pool<Postgres>,
    embedded_dataset_ids: &[i32],
) -> Result<std::collections::HashMap<i32, EmbeddedDatasetStats>> {
    use std::collections::HashMap;

    let mut stats_map = HashMap::new();

    for &id in embedded_dataset_ids {
        match get_embedded_dataset_stats(pool, id).await {
            Ok(stats) => {
                stats_map.insert(id, stats);
            }
            Err(e) => {
                tracing::warn!("Failed to get stats for embedded dataset {}: {}", id, e);
            }
        }
    }

    Ok(stats_map)
}

pub async fn get_processed_batches(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
) -> Result<Vec<EmbeddedDatasetProcessedBatch>> {
    let batches = sqlx::query_as::<_, EmbeddedDatasetProcessedBatch>(GET_PROCESSED_BATCHES_QUERY)
        .bind(embedded_dataset_id)
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

/// Update the last_processed_at timestamp to a specific value
/// This prevents race conditions where items created between query and update are missed
pub async fn update_embedded_dataset_last_processed_at_to(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(UPDATE_EMBEDDED_DATASET_LAST_PROCESSED_AT_QUERY)
        .bind(embedded_dataset_id)
        .bind(timestamp)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_embedded_dataset_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    title: &str,
    dataset_transform_id: i32,
    source_dataset_id: i32,
    embedder_id: i32,
    owner: &OwnerInfo,
    collection_name: &str,
) -> Result<EmbeddedDataset> {
    let mut embedded_dataset = sqlx::query_as::<_, EmbeddedDataset>(CREATE_EMBEDDED_DATASET_QUERY)
        .bind(title)
        .bind(dataset_transform_id)
        .bind(source_dataset_id)
        .bind(embedder_id)
        .bind(&owner.owner_id)
        .bind(&owner.owner_display_name)
        .bind(collection_name)
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
) -> Result<Vec<EmbeddedDataset>> {
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_FOR_TRANSFORM_QUERY)
            .bind(dataset_transform_id)
            .fetch_all(executor)
            .await?;
    Ok(embedded_datasets)
}

pub async fn delete_embedded_dataset_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    embedded_dataset_id: i32,
) -> Result<()> {
    sqlx::query(DELETE_EMBEDDED_DATASET_QUERY)
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
