use anyhow::Result;
use sqlx::{Pool, Postgres};

use crate::transforms::models::{ProcessedFile, Transform, TransformStats};

pub(crate) struct CreateTransformParams<'a> {
    pub(crate) title: &'a str,
    pub(crate) collection_id: Option<i32>,
    pub(crate) dataset_id: i32,
    pub(crate) owner: &'a str,
    pub(crate) chunk_size: i32,
    pub(crate) job_type: &'a str,
    pub(crate) source_dataset_id: Option<i32>,
    pub(crate) target_dataset_id: Option<i32>,
    pub(crate) embedder_ids: Option<Vec<i32>>,
    pub(crate) job_config: &'a serde_json::Value,
    pub(crate) collection_mappings: &'a serde_json::Value,
}

const GET_TRANSFORM_QUERY: &str = r#"
    SELECT transform_id, title, collection_id, dataset_id, owner, is_enabled, chunk_size, 
           job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings, created_at, updated_at
    FROM transforms
    WHERE owner = $1 AND transform_id = $2
"#;

const GET_TRANSFORM_BY_ID_QUERY: &str = r#"
    SELECT transform_id, title, collection_id, dataset_id, owner, is_enabled, chunk_size, 
           job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings, created_at, updated_at
    FROM transforms
    WHERE transform_id = $1
"#;

const GET_TRANSFORMS_QUERY: &str = r#"
    SELECT transform_id, title, collection_id, dataset_id, owner, is_enabled, chunk_size, 
           job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings, created_at, updated_at
    FROM transforms
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

const GET_ACTIVE_TRANSFORMS_QUERY: &str = r#"
    SELECT transform_id, title, collection_id, dataset_id, owner, is_enabled, chunk_size, 
           job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings, created_at, updated_at
    FROM transforms
    WHERE is_enabled = TRUE
    ORDER BY created_at DESC
"#;

const CREATE_TRANSFORM_QUERY: &str = r#"
    INSERT INTO transforms (title, collection_id, dataset_id, owner, is_enabled, chunk_size,
                            job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings)
    VALUES ($1, $2, $3, $4, TRUE, $5, $6, $7, $8, $9, $10, $11)
    RETURNING transform_id, title, collection_id, dataset_id, owner, is_enabled, chunk_size, 
              job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings, created_at, updated_at
"#;

const UPDATE_TRANSFORM_QUERY: &str = r#"
    UPDATE transforms
    SET title = COALESCE($3, title),
        is_enabled = COALESCE($4, is_enabled),
        chunk_size = COALESCE($5, chunk_size),
        embedder_ids = COALESCE($6, embedder_ids),
        updated_at = NOW()
    WHERE owner = $1 AND transform_id = $2
    RETURNING transform_id, title, collection_id, dataset_id, owner, is_enabled, chunk_size, 
              job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings, created_at, updated_at
"#;

const DELETE_TRANSFORM_QUERY: &str = r#"
    DELETE FROM transforms
    WHERE owner = $1 AND transform_id = $2
"#;

const UPDATE_COLLECTION_MAPPINGS_QUERY: &str = r#"
    UPDATE transforms
    SET collection_mappings = $3,
        updated_at = NOW()
    WHERE owner = $1 AND transform_id = $2
    RETURNING transform_id, title, collection_id, dataset_id, owner, is_enabled, chunk_size, 
              job_type, source_dataset_id, target_dataset_id, embedder_ids, job_config, collection_mappings, created_at, updated_at
"#;

const MARK_FILE_PROCESSED_QUERY: &str = r#"
    INSERT INTO transform_processed_files (transform_id, file_key, item_count, process_status, processing_duration_ms)
    VALUES ($1, $2, $3, 'completed', $4)
    ON CONFLICT (transform_id, file_key)
    DO UPDATE SET
        processed_at = NOW(),
        item_count = EXCLUDED.item_count,
        process_status = 'completed',
        process_error = NULL,
        processing_duration_ms = EXCLUDED.processing_duration_ms
"#;

const MARK_FILE_FAILED_QUERY: &str = r#"
    INSERT INTO transform_processed_files (transform_id, file_key, item_count, process_status, process_error)
    VALUES ($1, $2, 0, 'failed', $3)
    ON CONFLICT (transform_id, file_key)
    DO UPDATE SET
        processed_at = NOW(),
        process_status = 'failed',
        process_error = EXCLUDED.process_error
"#;

const GET_PROCESSED_FILES_QUERY: &str = r#"
    SELECT id, transform_id, file_key, processed_at, item_count, process_status, process_error, processing_duration_ms
    FROM transform_processed_files
    WHERE transform_id = $1
    ORDER BY processed_at DESC
"#;

const GET_TRANSFORM_STATS_QUERY: &str = r#"
    SELECT
        transform_id,
        COUNT(*) as total_files_processed,
        SUM(CASE WHEN process_status = 'completed' THEN 1 ELSE 0 END) as successful_files,
        SUM(CASE WHEN process_status = 'failed' THEN 1 ELSE 0 END) as failed_files,
        SUM(CASE WHEN process_status = 'completed' THEN item_count ELSE 0 END) as total_items_created
    FROM transform_processed_files
    WHERE transform_id = $1
    GROUP BY transform_id
"#;

const GET_TRANSFORM_STATS_ENHANCED_QUERY: &str = r#"
    SELECT
        transform_id,
        COUNT(*) as total_items_processed,
        SUM(CASE WHEN process_status = 'completed' THEN 1 ELSE 0 END) as successful_items,
        SUM(CASE WHEN process_status = 'failed' THEN 1 ELSE 0 END) as failed_items,
        SUM(CASE WHEN process_status = 'completed' THEN item_count ELSE 0 END) as total_chunks_embedded,
        SUM(CASE WHEN process_status = 'failed' THEN item_count ELSE 0 END) as total_chunks_failed
    FROM transform_processed_files
    WHERE transform_id = $1
    GROUP BY transform_id
"#;

pub(crate) async fn get_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    transform_id: i32,
) -> Result<Transform> {
    let transform: Transform = sqlx::query_as::<_, Transform>(GET_TRANSFORM_QUERY)
        .bind(owner)
        .bind(transform_id)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

pub(crate) async fn get_transform_by_id(
    pool: &Pool<Postgres>,
    transform_id: i32,
) -> Result<Transform> {
    let transform: Transform = sqlx::query_as::<_, Transform>(GET_TRANSFORM_BY_ID_QUERY)
        .bind(transform_id)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

pub(crate) async fn get_transforms(pool: &Pool<Postgres>, owner: &str) -> Result<Vec<Transform>> {
    let transforms: Vec<Transform> = sqlx::query_as::<_, Transform>(GET_TRANSFORMS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await?;
    Ok(transforms)
}

pub(crate) async fn get_active_transforms(pool: &Pool<Postgres>) -> Result<Vec<Transform>> {
    let transforms: Vec<Transform> = sqlx::query_as::<_, Transform>(GET_ACTIVE_TRANSFORMS_QUERY)
        .fetch_all(pool)
        .await?;
    Ok(transforms)
}

pub(crate) async fn create_transform(
    pool: &Pool<Postgres>,
    params: CreateTransformParams<'_>,
) -> Result<Transform> {
    let transform: Transform = sqlx::query_as::<_, Transform>(CREATE_TRANSFORM_QUERY)
        .bind(params.title)
        .bind(params.collection_id)
        .bind(params.dataset_id)
        .bind(params.owner)
        .bind(params.chunk_size)
        .bind(params.job_type)
        .bind(params.source_dataset_id)
        .bind(params.target_dataset_id)
        .bind(params.embedder_ids.as_deref())
        .bind(params.job_config)
        .bind(params.collection_mappings)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

pub(crate) async fn update_transform(
    pool: &Pool<Postgres>,
    transform_id: i32,
    owner: &str,
    title: Option<&str>,
    enabled: Option<bool>,
    chunk_size: Option<i32>,
    embedder_ids: Option<Vec<i32>>,
) -> Result<Transform> {
    let transform: Transform = sqlx::query_as::<_, Transform>(UPDATE_TRANSFORM_QUERY)
        .bind(owner)
        .bind(transform_id)
        .bind(title)
        .bind(enabled)
        .bind(chunk_size)
        .bind(embedder_ids)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

pub(crate) async fn delete_transform(
    pool: &Pool<Postgres>,
    transform_id: i32,
    owner: &str,
) -> Result<()> {
    sqlx::query(DELETE_TRANSFORM_QUERY)
        .bind(owner)
        .bind(transform_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn update_collection_mappings(
    pool: &Pool<Postgres>,
    transform_id: i32,
    owner: &str,
    collection_mappings: &serde_json::Value,
) -> Result<Transform> {
    let transform: Transform = sqlx::query_as::<_, Transform>(UPDATE_COLLECTION_MAPPINGS_QUERY)
        .bind(owner)
        .bind(transform_id)
        .bind(collection_mappings)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

pub(crate) async fn mark_file_processed(
    pool: &Pool<Postgres>,
    transform_id: i32,
    file_key: &str,
    item_count: i32,
    processing_duration_ms: Option<i64>,
) -> Result<()> {
    sqlx::query(MARK_FILE_PROCESSED_QUERY)
        .bind(transform_id)
        .bind(file_key)
        .bind(item_count)
        .bind(processing_duration_ms)
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn mark_file_failed(
    pool: &Pool<Postgres>,
    transform_id: i32,
    file_key: &str,
    process_error: &str,
) -> Result<()> {
    sqlx::query(MARK_FILE_FAILED_QUERY)
        .bind(transform_id)
        .bind(file_key)
        .bind(process_error)
        .execute(pool)
        .await?;
    Ok(())
}

pub(crate) async fn get_processed_files(
    pool: &Pool<Postgres>,
    transform_id: i32,
) -> Result<Vec<ProcessedFile>> {
    let files: Vec<ProcessedFile> = sqlx::query_as::<_, ProcessedFile>(GET_PROCESSED_FILES_QUERY)
        .bind(transform_id)
        .fetch_all(pool)
        .await?;
    Ok(files)
}

pub(crate) async fn get_transform_stats(
    pool: &Pool<Postgres>,
    transform_id: i32,
) -> Result<Option<TransformStats>> {
    let stats: Option<TransformStats> =
        sqlx::query_as::<_, TransformStats>(GET_TRANSFORM_STATS_QUERY)
            .bind(transform_id)
            .fetch_optional(pool)
            .await?;
    Ok(stats)
}

#[tracing::instrument(name = "get_transform_stats_enhanced", skip(pool), fields(transform_id = %transform_id))]
pub(crate) async fn get_transform_stats_enhanced(
    pool: &Pool<Postgres>,
    transform_id: i32,
) -> Result<Option<crate::transforms::models::TransformStatsEnhanced>> {
    let stats: Option<crate::transforms::models::TransformStatsEnhanced> =
        sqlx::query_as::<_, crate::transforms::models::TransformStatsEnhanced>(
            GET_TRANSFORM_STATS_ENHANCED_QUERY,
        )
        .bind(transform_id)
        .fetch_optional(pool)
        .await?;

    if let Some(ref s) = stats {
        tracing::debug!(
            total_items_processed = s.total_items_processed,
            successful_items = s.successful_items,
            failed_items = s.failed_items,
            total_chunks_embedded = s.total_chunks_embedded,
            total_chunks_failed = s.total_chunks_failed,
            "retrieved enhanced transform stats"
        );
    } else {
        tracing::debug!("no stats found for transform");
    }

    Ok(stats)
}
