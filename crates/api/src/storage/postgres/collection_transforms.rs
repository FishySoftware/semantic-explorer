use anyhow::Result;
use sqlx::{Pool, Postgres};

use crate::transforms::collection::models::{
    CollectionTransform, CollectionTransformStats, ProcessedFile,
};
use semantic_explorer_core::models::PaginatedResponse;

fn validate_sort_field(sort_by: &str) -> Result<String> {
    match sort_by {
        "title" | "is_enabled" | "created_at" | "updated_at" | "chunk_size" => {
            Ok(sort_by.to_string())
        }
        _ => anyhow::bail!("Invalid sort field: {}", sort_by),
    }
}

fn validate_sort_direction(direction: &str) -> Result<String> {
    match direction.to_lowercase().as_str() {
        "asc" | "desc" => Ok(direction.to_uppercase()),
        _ => anyhow::bail!("Invalid sort direction: {}", direction),
    }
}

const GET_COLLECTION_TRANSFORM_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE collection_transform_id = $1
"#;

const GET_COLLECTION_TRANSFORMS_FOR_COLLECTION_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE collection_id = $1
    ORDER BY created_at DESC
"#;

const GET_ACTIVE_COLLECTION_TRANSFORMS_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE is_enabled = TRUE
    ORDER BY created_at DESC
"#;

const CREATE_COLLECTION_TRANSFORM_QUERY: &str = r#"
    INSERT INTO collection_transforms (title, collection_id, dataset_id, owner, chunk_size, job_config)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING collection_transform_id, title, collection_id, dataset_id, owner, is_enabled,
              chunk_size, job_config, created_at, updated_at
"#;

const UPDATE_COLLECTION_TRANSFORM_QUERY: &str = r#"
    UPDATE collection_transforms
    SET title = COALESCE($3, title),
        is_enabled = COALESCE($4, is_enabled),
        chunk_size = COALESCE($5, chunk_size),
        job_config = COALESCE($6, job_config),
        updated_at = NOW()
    WHERE collection_transform_id = $1
    RETURNING collection_transform_id, title, collection_id, dataset_id, owner, is_enabled,
              chunk_size, job_config, created_at, updated_at
"#;

const DELETE_COLLECTION_TRANSFORM_QUERY: &str = r#"
    DELETE FROM collection_transforms
    WHERE collection_transform_id = $1
"#;

const GET_COLLECTION_TRANSFORM_STATS_QUERY: &str = r#"
    SELECT
        $1::INTEGER as collection_transform_id,
        COUNT(*) as total_files_processed,
        COUNT(*) FILTER (WHERE process_status = 'completed') as successful_files,
        COUNT(*) FILTER (WHERE process_status = 'failed') as failed_files,
        COALESCE(SUM(item_count) FILTER (WHERE process_status = 'completed'), 0) as total_items_created,
        MAX(processed_at) as last_run_at
    FROM transform_processed_files
    WHERE transform_type = 'collection' AND transform_id = $1
"#;

const GET_PROCESSED_FILES_QUERY: &str = r#"
    SELECT id, transform_type, transform_id, file_key, processed_at, item_count,
           process_status, process_error, processing_duration_ms
    FROM transform_processed_files
    WHERE transform_type = 'collection' AND transform_id = $1
    ORDER BY processed_at DESC
"#;

const RECORD_PROCESSED_FILE_QUERY: &str = r#"
    INSERT INTO transform_processed_files
        (transform_type, transform_id, file_key, item_count, process_status, process_error, processing_duration_ms)
    VALUES ('collection', $1, $2, $3, $4, $5, $6)
    ON CONFLICT (transform_type, transform_id, file_key)
    DO UPDATE SET
        item_count = EXCLUDED.item_count,
        process_status = EXCLUDED.process_status,
        process_error = EXCLUDED.process_error,
        processing_duration_ms = EXCLUDED.processing_duration_ms,
        processed_at = NOW()
"#;

const CHECK_FILE_PROCESSED_QUERY: &str = r#"
    SELECT process_status
    FROM transform_processed_files
    WHERE transform_type = 'collection' AND transform_id = $1 AND file_key = $2
    LIMIT 1
"#;

const COUNT_COLLECTION_TRANSFORMS_QUERY: &str =
    "SELECT COUNT(*) as count FROM collection_transforms WHERE owner = $1";
const COUNT_COLLECTION_TRANSFORMS_WITH_SEARCH_QUERY: &str =
    "SELECT COUNT(*) as count FROM collection_transforms WHERE title ILIKE $1 AND owner = $2";

// Note: ORDER BY clause is built dynamically with validated identifiers
// Column names cannot be parameterized in PostgreSQL, so we validate and use format!
const GET_COLLECTION_TRANSFORMS_PAGINATED_BASE: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner = $1
"#;

const GET_COLLECTION_TRANSFORMS_PAGINATED_WITH_SEARCH_BASE: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1
    AND owner = $2
"#;

// CRUD operations
pub async fn get_collection_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_transform_id: i32,
) -> Result<CollectionTransform> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let transform = sqlx::query_as::<_, CollectionTransform>(GET_COLLECTION_TRANSFORM_QUERY)
        .bind(collection_transform_id)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(transform)
}

pub async fn get_collection_transforms_paginated(
    pool: &Pool<Postgres>,
    owner: &str,
    limit: i64,
    offset: i64,
    sort_by: &str,
    sort_direction: &str,
    search: Option<&str>,
) -> Result<PaginatedResponse<CollectionTransform>> {
    // Validate identifiers against allowlist to prevent SQL injection
    let sort_field = validate_sort_field(sort_by)?;
    let sort_dir = validate_sort_direction(sort_direction)?;

    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let (total_count, transforms) = if let Some(search_term) = search {
        let search_pattern = format!("%{}%", search_term);

        let count_result: (i64,) = sqlx::query_as(COUNT_COLLECTION_TRANSFORMS_WITH_SEARCH_QUERY)
            .bind(&search_pattern)
            .bind(owner)
            .fetch_one(&mut *tx)
            .await?;
        let total = count_result.0;

        // Build query with validated identifiers (column names cannot be parameterized)
        let query_str = format!(
            "{} ORDER BY {} {} LIMIT $3 OFFSET $4",
            GET_COLLECTION_TRANSFORMS_PAGINATED_WITH_SEARCH_BASE, sort_field, sort_dir
        );

        let items = sqlx::query_as::<_, CollectionTransform>(&query_str)
            .bind(&search_pattern)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;

        (total, items)
    } else {
        let count_result: (i64,) = sqlx::query_as(COUNT_COLLECTION_TRANSFORMS_QUERY)
            .bind(owner)
            .fetch_one(&mut *tx)
            .await?;
        let total = count_result.0;

        // Build query with validated identifiers (column names cannot be parameterized)
        let query_str = format!(
            "{} ORDER BY {} {} LIMIT $2 OFFSET $3",
            GET_COLLECTION_TRANSFORMS_PAGINATED_BASE, sort_field, sort_dir
        );

        let items = sqlx::query_as::<_, CollectionTransform>(&query_str)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;

        (total, items)
    };

    tx.commit().await?;

    Ok(PaginatedResponse {
        items: transforms,
        total_count,
        limit,
        offset,
    })
}

pub async fn get_collection_transforms_for_collection(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_id: i32,
) -> Result<Vec<CollectionTransform>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let transforms =
        sqlx::query_as::<_, CollectionTransform>(GET_COLLECTION_TRANSFORMS_FOR_COLLECTION_QUERY)
            .bind(collection_id)
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(transforms)
}

pub async fn get_active_collection_transforms(
    pool: &Pool<Postgres>,
) -> Result<Vec<CollectionTransform>> {
    let transforms =
        sqlx::query_as::<_, CollectionTransform>(GET_ACTIVE_COLLECTION_TRANSFORMS_QUERY)
            .fetch_all(pool)
            .await?;
    Ok(transforms)
}

pub async fn create_collection_transform(
    pool: &Pool<Postgres>,
    title: &str,
    collection_id: i32,
    dataset_id: i32,
    owner: &str,
    chunk_size: i32,
    job_config: &serde_json::Value,
) -> Result<CollectionTransform> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let transform = sqlx::query_as::<_, CollectionTransform>(CREATE_COLLECTION_TRANSFORM_QUERY)
        .bind(title)
        .bind(collection_id)
        .bind(dataset_id)
        .bind(owner)
        .bind(chunk_size)
        .bind(job_config)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(transform)
}

pub async fn update_collection_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_transform_id: i32,
    title: Option<&str>,
    is_enabled: Option<bool>,
    chunk_size: Option<i32>,
    job_config: Option<&serde_json::Value>,
) -> Result<CollectionTransform> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let transform = sqlx::query_as::<_, CollectionTransform>(UPDATE_COLLECTION_TRANSFORM_QUERY)
        .bind(collection_transform_id)
        .bind(title)
        .bind(is_enabled)
        .bind(chunk_size)
        .bind(job_config)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(transform)
}

pub async fn delete_collection_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_transform_id: i32,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    sqlx::query(DELETE_COLLECTION_TRANSFORM_QUERY)
        .bind(collection_transform_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn get_collection_transform_stats(
    pool: &Pool<Postgres>,
    collection_transform_id: i32,
) -> Result<CollectionTransformStats> {
    let stats = sqlx::query_as::<_, CollectionTransformStats>(GET_COLLECTION_TRANSFORM_STATS_QUERY)
        .bind(collection_transform_id)
        .fetch_one(pool)
        .await?;
    Ok(stats)
}

pub async fn get_processed_files(
    pool: &Pool<Postgres>,
    collection_transform_id: i32,
) -> Result<Vec<ProcessedFile>> {
    let files = sqlx::query_as::<_, ProcessedFile>(GET_PROCESSED_FILES_QUERY)
        .bind(collection_transform_id)
        .fetch_all(pool)
        .await?;
    Ok(files)
}

pub async fn record_processed_file(
    pool: &Pool<Postgres>,
    collection_transform_id: i32,
    file_key: &str,
    item_count: i32,
    process_status: &str,
    process_error: Option<&str>,
    processing_duration_ms: Option<i64>,
) -> Result<()> {
    sqlx::query(RECORD_PROCESSED_FILE_QUERY)
        .bind(collection_transform_id)
        .bind(file_key)
        .bind(item_count)
        .bind(process_status)
        .bind(process_error)
        .bind(processing_duration_ms)
        .execute(pool)
        .await?;
    Ok(())
}

/// Check if a file was already successfully processed for this collection transform
/// Returns true if the file was processed with status 'completed'
pub async fn is_file_already_processed(
    pool: &Pool<Postgres>,
    collection_transform_id: i32,
    file_key: &str,
) -> Result<bool> {
    let result: Option<(String,)> = sqlx::query_as(CHECK_FILE_PROCESSED_QUERY)
        .bind(collection_transform_id)
        .bind(file_key)
        .fetch_optional(pool)
        .await?;

    Ok(result
        .map(|(status,)| status == "completed")
        .unwrap_or(false))
}
