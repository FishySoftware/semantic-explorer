use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::transforms::collection::models::{
    CollectionTransform, CollectionTransformStats, FailedFileWithTransform, ProcessedFile,
};
use semantic_explorer_core::models::PaginatedResponse;
use semantic_explorer_core::observability::record_database_query;
use semantic_explorer_core::owner_info::OwnerInfo;

fn validate_sort_field(sort_by: &str) -> Result<&'static str> {
    match sort_by {
        "title" => Ok("title"),
        "is_enabled" => Ok("is_enabled"),
        "created_at" => Ok("created_at"),
        "updated_at" => Ok("updated_at"),
        "chunk_size" => Ok("chunk_size"),
        _ => anyhow::bail!("Invalid sort field: {}", sort_by),
    }
}

fn validate_sort_direction(direction: &str) -> Result<&'static str> {
    match direction.to_lowercase().as_str() {
        "asc" => Ok("ASC"),
        "desc" => Ok("DESC"),
        _ => anyhow::bail!("Invalid sort direction: {}", direction),
    }
}

const GET_COLLECTION_TRANSFORM_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE collection_transform_id = $1 AND owner_id = $2
"#;

const GET_COLLECTION_TRANSFORM_PRIVILEGED_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE collection_transform_id = $1
"#;

const GET_COLLECTION_TRANSFORMS_FOR_COLLECTION_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE collection_id = $1 AND owner_id = $2
    ORDER BY created_at DESC
"#;

const GET_COLLECTION_TRANSFORMS_FOR_DATASET_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE dataset_id = $1 AND owner_id = $2
    ORDER BY created_at DESC
"#;

const GET_ACTIVE_COLLECTION_TRANSFORMS_QUERY: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE is_enabled = TRUE
    ORDER BY created_at DESC
"#;

const CREATE_COLLECTION_TRANSFORM_QUERY: &str = r#"
    INSERT INTO collection_transforms (title, collection_id, dataset_id, owner_id, owner_display_name, chunk_size, job_config)
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    RETURNING collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
              chunk_size, job_config, created_at, updated_at
"#;

const UPDATE_COLLECTION_TRANSFORM_QUERY: &str = r#"
    UPDATE collection_transforms
    SET title = COALESCE($2, title),
        is_enabled = COALESCE($3, is_enabled),
        chunk_size = COALESCE($4, chunk_size),
        job_config = COALESCE($5, job_config),
        updated_at = NOW()
    WHERE collection_transform_id = $1 AND owner_id = $6
    RETURNING collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
              chunk_size, job_config, created_at, updated_at
"#;

const DELETE_COLLECTION_TRANSFORM_QUERY: &str = r#"
    DELETE FROM collection_transforms
    WHERE collection_transform_id = $1 AND owner_id = $2
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

/// Batch query to fetch stats for multiple collection transforms in a single round-trip
/// Eliminates N+1 pattern by using ANY($1) instead of individual queries
const GET_BATCH_COLLECTION_TRANSFORM_STATS_QUERY: &str = r#"
    SELECT
        transform_id as collection_transform_id,
        COUNT(*) as total_files_processed,
        COUNT(*) FILTER (WHERE process_status = 'completed') as successful_files,
        COUNT(*) FILTER (WHERE process_status = 'failed') as failed_files,
        COALESCE(SUM(item_count) FILTER (WHERE process_status = 'completed'), 0) as total_items_created,
        MAX(processed_at) as last_run_at
    FROM transform_processed_files
    WHERE transform_type = 'collection' AND transform_id = ANY($1)
    GROUP BY transform_id
"#;

/// Batch query to verify ownership of multiple collection transforms in a single query
const VERIFY_COLLECTION_TRANSFORMS_OWNERSHIP_BATCH_QUERY: &str = r#"
    SELECT collection_transform_id
    FROM collection_transforms
    WHERE collection_transform_id = ANY($1) AND owner_id = $2
"#;

const GET_FAILED_FILES_FOR_COLLECTION_QUERY: &str = r#"
    SELECT tpf.id, tpf.transform_type, tpf.transform_id, tpf.file_key, tpf.processed_at,
           tpf.item_count, tpf.process_status, tpf.process_error, tpf.processing_duration_ms,
           ct.title as transform_title
    FROM transform_processed_files tpf
    INNER JOIN collection_transforms ct ON ct.collection_transform_id = tpf.transform_id
    WHERE tpf.transform_type = 'collection'
      AND ct.collection_id = $1
      AND ct.owner_id = $2
      AND tpf.process_status = 'failed'
    ORDER BY tpf.processed_at DESC
    LIMIT $3 OFFSET $4
"#;

const COUNT_FAILED_FILES_FOR_COLLECTION_QUERY: &str = r#"
    SELECT COUNT(*)::BIGINT as count
    FROM transform_processed_files tpf
    INNER JOIN collection_transforms ct ON ct.collection_transform_id = tpf.transform_id
    WHERE tpf.transform_type = 'collection'
      AND ct.collection_id = $1
      AND ct.owner_id = $2
      AND tpf.process_status = 'failed'
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
    "SELECT COUNT(*) as count FROM collection_transforms WHERE owner_id = $1";
const COUNT_COLLECTION_TRANSFORMS_WITH_SEARCH_QUERY: &str =
    "SELECT COUNT(*) as count FROM collection_transforms WHERE title ILIKE $1 AND owner_id = $2";

// Static sort query variants for plan caching
// Each sort field/direction combination is a separate const
const CT_PAGINATED_TITLE_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY title ASC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_TITLE_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY title DESC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_IS_ENABLED_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY is_enabled ASC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_IS_ENABLED_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY is_enabled DESC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_CREATED_AT_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY created_at ASC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_CREATED_AT_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY created_at DESC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_UPDATED_AT_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY updated_at ASC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_UPDATED_AT_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY updated_at DESC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_CHUNK_SIZE_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY chunk_size ASC LIMIT $2 OFFSET $3
"#;
const CT_PAGINATED_CHUNK_SIZE_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE owner_id = $1
    ORDER BY chunk_size DESC LIMIT $2 OFFSET $3
"#;

const CT_SEARCH_TITLE_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY title ASC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_TITLE_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY title DESC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_IS_ENABLED_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY is_enabled ASC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_IS_ENABLED_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY is_enabled DESC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_CREATED_AT_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY created_at ASC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_CREATED_AT_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY created_at DESC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_UPDATED_AT_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY updated_at ASC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_UPDATED_AT_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY updated_at DESC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_CHUNK_SIZE_ASC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY chunk_size ASC LIMIT $3 OFFSET $4
"#;
const CT_SEARCH_CHUNK_SIZE_DESC: &str = r#"
    SELECT collection_transform_id, title, collection_id, dataset_id, owner_id, owner_display_name, is_enabled,
           chunk_size, job_config, created_at, updated_at
    FROM collection_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY chunk_size DESC LIMIT $3 OFFSET $4
"#;

fn get_ct_paginated_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("title", "ASC") => CT_PAGINATED_TITLE_ASC,
        ("title", "DESC") => CT_PAGINATED_TITLE_DESC,
        ("is_enabled", "ASC") => CT_PAGINATED_IS_ENABLED_ASC,
        ("is_enabled", "DESC") => CT_PAGINATED_IS_ENABLED_DESC,
        ("created_at", "ASC") => CT_PAGINATED_CREATED_AT_ASC,
        ("updated_at", "ASC") => CT_PAGINATED_UPDATED_AT_ASC,
        ("updated_at", "DESC") => CT_PAGINATED_UPDATED_AT_DESC,
        ("chunk_size", "ASC") => CT_PAGINATED_CHUNK_SIZE_ASC,
        ("chunk_size", "DESC") => CT_PAGINATED_CHUNK_SIZE_DESC,
        _ => CT_PAGINATED_CREATED_AT_DESC, // default
    }
}

fn get_ct_search_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("title", "ASC") => CT_SEARCH_TITLE_ASC,
        ("title", "DESC") => CT_SEARCH_TITLE_DESC,
        ("is_enabled", "ASC") => CT_SEARCH_IS_ENABLED_ASC,
        ("is_enabled", "DESC") => CT_SEARCH_IS_ENABLED_DESC,
        ("created_at", "ASC") => CT_SEARCH_CREATED_AT_ASC,
        ("updated_at", "ASC") => CT_SEARCH_UPDATED_AT_ASC,
        ("updated_at", "DESC") => CT_SEARCH_UPDATED_AT_DESC,
        ("chunk_size", "ASC") => CT_SEARCH_CHUNK_SIZE_ASC,
        ("chunk_size", "DESC") => CT_SEARCH_CHUNK_SIZE_DESC,
        _ => CT_SEARCH_CREATED_AT_DESC, // default
    }
}

// CRUD operations
pub async fn get_collection_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_transform_id: i32,
) -> Result<CollectionTransform> {
    let start = Instant::now();

    let result = sqlx::query_as::<_, CollectionTransform>(GET_COLLECTION_TRANSFORM_QUERY)
        .bind(collection_transform_id)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "collection_transforms", duration, result.is_ok());

    let transform = result?;
    Ok(transform)
}

/// Used by scanner workers that need to process triggers for specific transforms.
pub async fn get_collection_transform_privileged(
    pool: &Pool<Postgres>,
    collection_transform_id: i32,
) -> Result<CollectionTransform> {
    let transform =
        sqlx::query_as::<_, CollectionTransform>(GET_COLLECTION_TRANSFORM_PRIVILEGED_QUERY)
            .bind(collection_transform_id)
            .fetch_one(pool)
            .await?;
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

    let start = Instant::now();

    let (total_count, transforms) = if let Some(search_term) = search {
        let search_pattern = format!("%{}%", search_term);

        let count_result: (i64,) = sqlx::query_as(COUNT_COLLECTION_TRANSFORMS_WITH_SEARCH_QUERY)
            .bind(&search_pattern)
            .bind(owner)
            .fetch_one(pool)
            .await?;
        let total = count_result.0;

        // Use static query variant for plan caching
        let query_str = get_ct_search_query(sort_field, sort_dir);

        let items = sqlx::query_as::<_, CollectionTransform>(query_str)
            .bind(&search_pattern)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        (total, items)
    } else {
        let count_result: (i64,) = sqlx::query_as(COUNT_COLLECTION_TRANSFORMS_QUERY)
            .bind(owner)
            .fetch_one(pool)
            .await?;
        let total = count_result.0;

        // Use static query variant for plan caching
        let query_str = get_ct_paginated_query(sort_field, sort_dir);

        let items = sqlx::query_as::<_, CollectionTransform>(query_str)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        (total, items)
    };

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "collection_transforms", duration, true);

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
    let transforms =
        sqlx::query_as::<_, CollectionTransform>(GET_COLLECTION_TRANSFORMS_FOR_COLLECTION_QUERY)
            .bind(collection_id)
            .bind(owner)
            .fetch_all(pool)
            .await?;

    Ok(transforms)
}

pub async fn get_collection_transforms_for_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
) -> Result<Vec<CollectionTransform>> {
    let transforms =
        sqlx::query_as::<_, CollectionTransform>(GET_COLLECTION_TRANSFORMS_FOR_DATASET_QUERY)
            .bind(dataset_id)
            .bind(owner)
            .fetch_all(pool)
            .await?;

    Ok(transforms)
}

///
/// This function intentionally bypasses Row-Level Security to fetch ALL active
/// collection transforms across all users. It should ONLY be called by system
/// workers (collection-transforms worker) that need to process transforms for
/// all users
///
/// # Returns
/// All enabled collection transforms regardless of ownership
pub async fn get_active_collection_transforms_privileged(
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
    owner: &OwnerInfo,
    chunk_size: i32,
    job_config: &serde_json::Value,
) -> Result<CollectionTransform> {
    let transform = sqlx::query_as::<_, CollectionTransform>(CREATE_COLLECTION_TRANSFORM_QUERY)
        .bind(title)
        .bind(collection_id)
        .bind(dataset_id)
        .bind(&owner.owner_id)
        .bind(&owner.owner_display_name)
        .bind(chunk_size)
        .bind(job_config)
        .fetch_one(pool)
        .await?;

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
    let transform = sqlx::query_as::<_, CollectionTransform>(UPDATE_COLLECTION_TRANSFORM_QUERY)
        .bind(collection_transform_id)
        .bind(title)
        .bind(is_enabled)
        .bind(chunk_size)
        .bind(job_config)
        .bind(owner)
        .fetch_one(pool)
        .await?;

    Ok(transform)
}

pub async fn delete_collection_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_transform_id: i32,
) -> Result<()> {
    sqlx::query(DELETE_COLLECTION_TRANSFORM_QUERY)
        .bind(collection_transform_id)
        .bind(owner)
        .execute(pool)
        .await?;

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

/// Batch fetch stats for multiple collection transforms in a single query (eliminates N+1)
pub async fn get_batch_collection_transform_stats(
    pool: &Pool<Postgres>,
    collection_transform_ids: &[i32],
) -> Result<std::collections::HashMap<i32, CollectionTransformStats>> {
    use std::collections::HashMap;

    if collection_transform_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let stats_list =
        sqlx::query_as::<_, CollectionTransformStats>(GET_BATCH_COLLECTION_TRANSFORM_STATS_QUERY)
            .bind(collection_transform_ids)
            .fetch_all(pool)
            .await?;

    let stats_map: HashMap<i32, CollectionTransformStats> = stats_list
        .into_iter()
        .map(|s| (s.collection_transform_id, s))
        .collect();

    Ok(stats_map)
}

/// Verify ownership of multiple collection transforms in a single query (eliminates N+1)
/// Returns the list of IDs that the user owns
pub async fn verify_collection_transforms_ownership_batch(
    pool: &Pool<Postgres>,
    owner_id: &str,
    collection_transform_ids: &[i32],
) -> Result<Vec<i32>> {
    if collection_transform_ids.is_empty() {
        return Ok(Vec::new());
    }

    let owned_ids: Vec<(i32,)> = sqlx::query_as(VERIFY_COLLECTION_TRANSFORMS_OWNERSHIP_BATCH_QUERY)
        .bind(collection_transform_ids)
        .bind(owner_id)
        .fetch_all(pool)
        .await?;

    Ok(owned_ids.into_iter().map(|(id,)| id).collect())
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

pub async fn get_failed_files_for_collection(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_id: i32,
    limit: i64,
    offset: i64,
) -> Result<PaginatedResponse<FailedFileWithTransform>> {
    let count_result: (i64,) = sqlx::query_as(COUNT_FAILED_FILES_FOR_COLLECTION_QUERY)
        .bind(collection_id)
        .bind(owner)
        .fetch_one(pool)
        .await?;
    let total_count = count_result.0;

    let files = sqlx::query_as::<_, FailedFileWithTransform>(GET_FAILED_FILES_FOR_COLLECTION_QUERY)
        .bind(collection_id)
        .bind(owner)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(PaginatedResponse {
        items: files,
        total_count,
        limit,
        offset,
    })
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
