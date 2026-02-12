use crate::embedded_datasets::EmbeddedDataset;
use crate::storage::postgres::INTERNAL_BATCH_SIZE;
use crate::storage::postgres::embedded_datasets::{
    delete_embedded_dataset_in_transaction, get_embedded_datasets_for_transform_in_transaction,
};
use crate::transforms::dataset::models::{DatasetTransform, DatasetTransformStats};
use anyhow::{Context, Result};
use semantic_explorer_core::models::PaginatedResponse;
use semantic_explorer_core::observability::record_database_query;
use semantic_explorer_core::owner_info::OwnerInfo;
use sqlx::{Pool, Postgres, Transaction};
use std::time::Instant;

fn validate_sort_field(sort_by: &str) -> Result<&'static str> {
    match sort_by {
        "title" => Ok("title"),
        "is_enabled" => Ok("is_enabled"),
        "created_at" => Ok("created_at"),
        "updated_at" => Ok("updated_at"),
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

const GET_SOURCE_DATASET_TOTAL_CHUNKS_QUERY: &str = r#"
    SELECT total_chunks FROM datasets WHERE dataset_id = $1
"#;

const GET_DATASET_TRANSFORM_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE dataset_transform_id = $1 AND owner_id = $2
"#;

const GET_DATASET_TRANSFORM_PRIVILEGED_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE dataset_transform_id = $1
"#;

const GET_DATASET_TRANSFORMS_FOR_DATASET_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE source_dataset_id = $1 AND owner_id = $2
    ORDER BY created_at DESC
"#;

const GET_ACTIVE_DATASET_TRANSFORMS_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE is_enabled = TRUE
    ORDER BY created_at DESC
"#;

const CREATE_DATASET_TRANSFORM_QUERY: &str = r#"
    INSERT INTO dataset_transforms (title, source_dataset_id, embedder_ids, owner_id, owner_display_name, job_config)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
              job_config, created_at, updated_at
"#;

const UPDATE_DATASET_TRANSFORM_QUERY: &str = r#"
    UPDATE dataset_transforms
    SET title = COALESCE($2, title),
        is_enabled = COALESCE($3, is_enabled),
        embedder_ids = COALESCE($4, embedder_ids),
        job_config = COALESCE($5, job_config),
        updated_at = NOW()
    WHERE dataset_transform_id = $1 AND owner_id = $6
    RETURNING dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
              job_config, created_at, updated_at
"#;

const DELETE_DATASET_TRANSFORM_QUERY: &str = r#"
    DELETE FROM dataset_transforms
    WHERE dataset_transform_id = $1 AND owner_id = $2
"#;

const COUNT_DATASET_TRANSFORMS_QUERY: &str =
    "SELECT COUNT(*) as count FROM dataset_transforms WHERE owner_id = $1";
const COUNT_DATASET_TRANSFORMS_WITH_SEARCH_QUERY: &str =
    "SELECT COUNT(*) as count FROM dataset_transforms WHERE title ILIKE $1 AND owner_id = $2";

// Static sort query variants for plan caching
// Each sort field/direction combination is a separate const
const DT_PAGINATED_TITLE_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY title ASC LIMIT $2 OFFSET $3
"#;
const DT_PAGINATED_TITLE_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY title DESC LIMIT $2 OFFSET $3
"#;
const DT_PAGINATED_IS_ENABLED_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY is_enabled ASC LIMIT $2 OFFSET $3
"#;
const DT_PAGINATED_IS_ENABLED_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY is_enabled DESC LIMIT $2 OFFSET $3
"#;
const DT_PAGINATED_CREATED_AT_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY created_at ASC LIMIT $2 OFFSET $3
"#;
const DT_PAGINATED_CREATED_AT_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY created_at DESC LIMIT $2 OFFSET $3
"#;
const DT_PAGINATED_UPDATED_AT_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY updated_at ASC LIMIT $2 OFFSET $3
"#;
const DT_PAGINATED_UPDATED_AT_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner_id = $1
    ORDER BY updated_at DESC LIMIT $2 OFFSET $3
"#;

const DT_SEARCH_TITLE_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY title ASC LIMIT $3 OFFSET $4
"#;
const DT_SEARCH_TITLE_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY title DESC LIMIT $3 OFFSET $4
"#;
const DT_SEARCH_IS_ENABLED_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY is_enabled ASC LIMIT $3 OFFSET $4
"#;
const DT_SEARCH_IS_ENABLED_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY is_enabled DESC LIMIT $3 OFFSET $4
"#;
const DT_SEARCH_CREATED_AT_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY created_at ASC LIMIT $3 OFFSET $4
"#;
const DT_SEARCH_CREATED_AT_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY created_at DESC LIMIT $3 OFFSET $4
"#;
const DT_SEARCH_UPDATED_AT_ASC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY updated_at ASC LIMIT $3 OFFSET $4
"#;
const DT_SEARCH_UPDATED_AT_DESC: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner_id, owner_display_name, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY updated_at DESC LIMIT $3 OFFSET $4
"#;

fn get_dt_paginated_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("title", "ASC") => DT_PAGINATED_TITLE_ASC,
        ("title", "DESC") => DT_PAGINATED_TITLE_DESC,
        ("is_enabled", "ASC") => DT_PAGINATED_IS_ENABLED_ASC,
        ("is_enabled", "DESC") => DT_PAGINATED_IS_ENABLED_DESC,
        ("created_at", "ASC") => DT_PAGINATED_CREATED_AT_ASC,
        ("updated_at", "ASC") => DT_PAGINATED_UPDATED_AT_ASC,
        ("updated_at", "DESC") => DT_PAGINATED_UPDATED_AT_DESC,
        _ => DT_PAGINATED_CREATED_AT_DESC, // default
    }
}

fn get_dt_search_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("title", "ASC") => DT_SEARCH_TITLE_ASC,
        ("title", "DESC") => DT_SEARCH_TITLE_DESC,
        ("is_enabled", "ASC") => DT_SEARCH_IS_ENABLED_ASC,
        ("is_enabled", "DESC") => DT_SEARCH_IS_ENABLED_DESC,
        ("created_at", "ASC") => DT_SEARCH_CREATED_AT_ASC,
        ("updated_at", "ASC") => DT_SEARCH_UPDATED_AT_ASC,
        ("updated_at", "DESC") => DT_SEARCH_UPDATED_AT_DESC,
        _ => DT_SEARCH_CREATED_AT_DESC, // default
    }
}

const VERIFY_DATASET_TRANSFORM_OWNERSHIP_QUERY: &str = "SELECT dataset_transform_id FROM dataset_transforms WHERE dataset_transform_id = ANY($1) AND owner_id = $2";

// Old expensive query removed - now using dataset_transform_stats table
// See dataset_transform_stats::get_stats() for the replacement

pub async fn get_dataset_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_transform_id: i32,
) -> Result<DatasetTransform> {
    let start = Instant::now();

    let result = sqlx::query_as::<_, DatasetTransform>(GET_DATASET_TRANSFORM_QUERY)
        .bind(dataset_transform_id)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "dataset_transforms", duration, result.is_ok());

    let transform = result?;
    Ok(transform)
}

/// Used by scanner workers that need to process triggers for specific transforms.
pub async fn get_dataset_transform_privileged(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
) -> Result<DatasetTransform> {
    let transform = sqlx::query_as::<_, DatasetTransform>(GET_DATASET_TRANSFORM_PRIVILEGED_QUERY)
        .bind(dataset_transform_id)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

pub async fn get_dataset_transforms_paginated(
    pool: &Pool<Postgres>,
    owner: &str,
    limit: i64,
    offset: i64,
    sort_by: &str,
    sort_direction: &str,
    search: Option<&str>,
) -> Result<PaginatedResponse<DatasetTransform>> {
    // Validate identifiers against allowlist to prevent SQL injection
    let sort_field = validate_sort_field(sort_by)?;
    let sort_dir = validate_sort_direction(sort_direction)?;

    let start = Instant::now();

    let (total_count, transforms) = if let Some(search_term) = search {
        let search_pattern = format!("%{}%", search_term);

        let count_result: (i64,) = sqlx::query_as(COUNT_DATASET_TRANSFORMS_WITH_SEARCH_QUERY)
            .bind(&search_pattern)
            .bind(owner)
            .fetch_one(pool)
            .await?;
        let total = count_result.0;

        // Use static query variant for plan caching
        let query_str = get_dt_search_query(sort_field, sort_dir);

        let items = sqlx::query_as::<_, DatasetTransform>(query_str)
            .bind(&search_pattern)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        (total, items)
    } else {
        let count_result: (i64,) = sqlx::query_as(COUNT_DATASET_TRANSFORMS_QUERY)
            .bind(owner)
            .fetch_one(pool)
            .await?;
        let total = count_result.0;

        // Use static query variant for plan caching
        let query_str = get_dt_paginated_query(sort_field, sort_dir);

        let items = sqlx::query_as::<_, DatasetTransform>(query_str)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        (total, items)
    };

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "dataset_transforms", duration, true);

    Ok(PaginatedResponse {
        items: transforms,
        total_count,
        limit,
        offset,
    })
}

pub async fn get_dataset_transforms_for_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
) -> Result<Vec<DatasetTransform>> {
    let transforms =
        sqlx::query_as::<_, DatasetTransform>(GET_DATASET_TRANSFORMS_FOR_DATASET_QUERY)
            .bind(dataset_id)
            .bind(owner)
            .fetch_all(pool)
            .await?;

    Ok(transforms)
}

///
/// This function intentionally bypasses Row-Level Security to fetch ALL active
/// dataset transforms across all users. It should ONLY be called by system
/// workers (dataset-transforms worker) that need to process transforms for
/// all users.
///
/// # Returns
/// All enabled dataset transforms regardless of ownership
pub async fn get_active_dataset_transforms_privileged(
    pool: &Pool<Postgres>,
) -> Result<Vec<DatasetTransform>> {
    let transforms = sqlx::query_as::<_, DatasetTransform>(GET_ACTIVE_DATASET_TRANSFORMS_QUERY)
        .fetch_all(pool)
        .await?;
    Ok(transforms)
}

pub async fn create_dataset_transform(
    pool: &Pool<Postgres>,
    title: &str,
    source_dataset_id: i32,
    embedder_ids: &[i32],
    owner: &OwnerInfo,
    job_config: &serde_json::Value,
) -> Result<(DatasetTransform, Vec<EmbeddedDataset>)> {
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;

    // Step 1: Get source dataset total_chunks to calculate total_chunks_to_process
    let total_chunks: i64 = sqlx::query_scalar(GET_SOURCE_DATASET_TOTAL_CHUNKS_QUERY)
        .bind(source_dataset_id)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to fetch source dataset total_chunks")?;

    // Step 2: Create the Dataset Transform
    let transform = sqlx::query_as::<_, DatasetTransform>(CREATE_DATASET_TRANSFORM_QUERY)
        .bind(title)
        .bind(source_dataset_id)
        .bind(embedder_ids.to_vec())
        .bind(&owner.owner_id)
        .bind(&owner.owner_display_name)
        .bind(job_config)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to create dataset transform")?;

    // Step 3: Initialize stats with total_chunks_to_process = source total_chunks * embedder_count
    let total_chunks_to_process = total_chunks * embedder_ids.len() as i64;
    super::dataset_transform_stats::initialize_stats(
        &mut tx,
        transform.dataset_transform_id,
        total_chunks_to_process,
    )
    .await
    .context("Failed to initialize dataset transform stats")?;

    // Step 4: Create N Embedded Datasets (one per embedder)
    let mut embedded_datasets = Vec::new();
    for embedder_id in embedder_ids {
        let embedded_dataset = create_embedded_dataset_internal(
            &mut tx,
            transform.dataset_transform_id,
            source_dataset_id,
            *embedder_id,
            owner,
            &transform.title,
        )
        .await
        .context(format!(
            "Failed to create embedded dataset for embedder {}",
            embedder_id
        ))?;
        embedded_datasets.push(embedded_dataset);
    }

    tx.commit().await.context("Failed to commit transaction")?;

    Ok((transform, embedded_datasets))
}

/// Updates a Dataset Transform and manages Embedded Datasets:
/// - If embedder_ids is updated, adds new embedded datasets for new embedders and removes old ones
pub async fn update_dataset_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_transform_id: i32,
    title: Option<&str>,
    is_enabled: Option<bool>,
    embedder_ids: Option<&[i32]>,
    job_config: Option<&serde_json::Value>,
) -> Result<(DatasetTransform, Vec<EmbeddedDataset>)> {
    let mut tx = pool.begin().await?;

    // Update the Dataset Transform
    let transform = sqlx::query_as::<_, DatasetTransform>(UPDATE_DATASET_TRANSFORM_QUERY)
        .bind(dataset_transform_id)
        .bind(title)
        .bind(is_enabled)
        .bind(embedder_ids.map(|ids| ids.to_vec()))
        .bind(job_config)
        .bind(owner)
        .fetch_one(&mut *tx)
        .await?;

    // If embedder_ids was updated, sync embedded datasets
    let embedded_datasets = if embedder_ids.is_some() {
        sync_embedded_datasets(&mut tx, &transform).await?
    } else {
        // Just fetch existing embedded datasets
        get_embedded_datasets_for_transform_internal(&mut tx, dataset_transform_id).await?
    };

    tx.commit().await?;

    Ok((transform, embedded_datasets))
}

pub async fn delete_dataset_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_transform_id: i32,
) -> Result<()> {
    // Cascading deletes will handle embedded datasets and processed files
    sqlx::query(DELETE_DATASET_TRANSFORM_QUERY)
        .bind(dataset_transform_id)
        .bind(owner)
        .execute(pool)
        .await?;

    Ok(())
}

// Batch ownership verification

/// Verifies ownership of multiple dataset transforms in a single query.
/// Returns the IDs that exist and are owned by the user.
pub async fn verify_dataset_transform_ownership(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_transform_ids: &[i32],
) -> Result<Vec<i32>> {
    if dataset_transform_ids.is_empty() {
        return Ok(Vec::new());
    }

    let owned_ids: Vec<(i32,)> = sqlx::query_as(VERIFY_DATASET_TRANSFORM_OWNERSHIP_QUERY)
        .bind(dataset_transform_ids)
        .bind(owner)
        .fetch_all(pool)
        .await?;

    Ok(owned_ids.into_iter().map(|(id,)| id).collect())
}

// Statistics

pub async fn get_dataset_transform_stats(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_transform_id: i32,
) -> Result<DatasetTransformStats> {
    // Use the new efficient stats table instead of expensive aggregation
    // If no stats row exists (old transforms), it will return zeros via LEFT JOIN
    let stats = super::dataset_transform_stats::get_stats(pool, owner, dataset_transform_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Dataset transform {} not found or access denied",
                dataset_transform_id
            )
        })?;
    Ok(stats)
}

pub async fn get_batch_dataset_transform_stats(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_transform_ids: &[i32],
) -> Result<std::collections::HashMap<i32, DatasetTransformStats>> {
    use std::collections::HashMap;

    if dataset_transform_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Use the new efficient stats table - single simple query with indexes
    let stats_list =
        super::dataset_transform_stats::get_batch_stats(pool, owner, dataset_transform_ids).await?;

    // Convert Vec to HashMap
    let stats_map = stats_list
        .into_iter()
        .map(|stats| (stats.dataset_transform_id, stats))
        .collect();

    Ok(stats_map)
}

// Helper functions

/// Creates an embedded dataset within a transaction
async fn create_embedded_dataset_internal(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    source_dataset_id: i32,
    embedder_id: i32,
    owner: &OwnerInfo,
    dataset_transform_title: &str,
) -> Result<EmbeddedDataset> {
    // Import the create function from embedded_datasets module
    use crate::storage::postgres::embedded_datasets::create_embedded_dataset_in_transaction;

    let title = format!("{dataset_transform_title}-{embedder_id}");
    let collection_name = EmbeddedDataset::generate_collection_name(0, &owner.owner_id); // Will be updated with actual ID

    create_embedded_dataset_in_transaction(
        tx,
        &title,
        dataset_transform_id,
        source_dataset_id,
        embedder_id,
        owner,
        &collection_name,
        None, // dimensions will be derived from embedder for transform-based datasets
    )
    .await
}

/// Syncs embedded datasets with the current embedder_ids in the transform
/// Adds new embedded datasets for new embedders, removes for embedders no longer in the list
async fn sync_embedded_datasets(
    tx: &mut Transaction<'_, Postgres>,
    transform: &DatasetTransform,
) -> Result<Vec<EmbeddedDataset>> {
    // Get existing embedded datasets
    let existing =
        get_embedded_datasets_for_transform_internal(&mut *tx, transform.dataset_transform_id)
            .await?;

    let existing_embedder_ids: Vec<i32> = existing.iter().map(|ed| ed.embedder_id).collect();

    // Find embedders to add (in new list but not in existing)
    let to_add: Vec<i32> = transform
        .embedder_ids
        .iter()
        .filter(|id| !existing_embedder_ids.contains(id))
        .copied()
        .collect();

    // Find embedders to remove (in existing but not in new list)
    let to_remove: Vec<i32> = existing_embedder_ids
        .iter()
        .filter(|id| !transform.embedder_ids.contains(id))
        .copied()
        .collect();

    // Add new embedded datasets
    let owner = OwnerInfo::new(
        transform.owner_id.clone(),
        transform.owner_display_name.clone(),
    );
    for embedder_id in to_add {
        create_embedded_dataset_internal(
            tx,
            transform.dataset_transform_id,
            transform.source_dataset_id,
            embedder_id,
            &owner,
            &transform.title,
        )
        .await?;
    }

    // Remove old embedded datasets
    for embedder_id in to_remove {
        if let Some(ed) = existing.iter().find(|ed| ed.embedder_id == embedder_id) {
            delete_embedded_dataset_internal(tx, ed.embedded_dataset_id).await?;
        }
    }

    // Return updated list
    get_embedded_datasets_for_transform_internal(&mut *tx, transform.dataset_transform_id).await
}

async fn get_embedded_datasets_for_transform_internal(
    executor: &mut sqlx::PgConnection,
    dataset_transform_id: i32,
) -> Result<Vec<EmbeddedDataset>> {
    // Embedded datasets per transform is a small set; a single batch suffices.
    get_embedded_datasets_for_transform_in_transaction(
        executor,
        dataset_transform_id,
        INTERNAL_BATCH_SIZE,
        0,
    )
    .await
}

async fn delete_embedded_dataset_internal(
    tx: &mut Transaction<'_, Postgres>,
    embedded_dataset_id: i32,
) -> Result<()> {
    delete_embedded_dataset_in_transaction(tx, embedded_dataset_id).await
}
