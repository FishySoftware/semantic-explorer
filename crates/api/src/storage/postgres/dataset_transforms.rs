use crate::embedded_datasets::EmbeddedDataset;
use crate::transforms::dataset::models::{DatasetTransform, DatasetTransformStats};
use anyhow::{Context, Result};
use sqlx::{Pool, Postgres, Transaction};

const GET_DATASET_TRANSFORM_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner = $1 AND dataset_transform_id = $2
"#;

const GET_DATASET_TRANSFORMS_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

const GET_DATASET_TRANSFORMS_FOR_DATASET_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE owner = $1 AND source_dataset_id = $2
    ORDER BY created_at DESC
"#;

const GET_ACTIVE_DATASET_TRANSFORMS_QUERY: &str = r#"
    SELECT dataset_transform_id, title, source_dataset_id, embedder_ids, owner, is_enabled,
           job_config, created_at, updated_at
    FROM dataset_transforms
    WHERE is_enabled = TRUE
    ORDER BY created_at DESC
"#;

const CREATE_DATASET_TRANSFORM_QUERY: &str = r#"
    INSERT INTO dataset_transforms (title, source_dataset_id, embedder_ids, owner, job_config)
    VALUES ($1, $2, $3, $4, $5)
    RETURNING dataset_transform_id, title, source_dataset_id, embedder_ids, owner, is_enabled,
              job_config, created_at, updated_at
"#;

const UPDATE_DATASET_TRANSFORM_QUERY: &str = r#"
    UPDATE dataset_transforms
    SET title = COALESCE($3, title),
        is_enabled = COALESCE($4, is_enabled),
        embedder_ids = COALESCE($5, embedder_ids),
        job_config = COALESCE($6, job_config),
        updated_at = NOW()
    WHERE owner = $1 AND dataset_transform_id = $2
    RETURNING dataset_transform_id, title, source_dataset_id, embedder_ids, owner, is_enabled,
              job_config, created_at, updated_at
"#;

const DELETE_DATASET_TRANSFORM_QUERY: &str = r#"
    DELETE FROM dataset_transforms
    WHERE owner = $1 AND dataset_transform_id = $2
"#;

const GET_DATASET_TRANSFORM_STATS_QUERY: &str = r#"
    WITH unique_batches AS (
        SELECT
            ed.dataset_transform_id,
            ed.embedded_dataset_id,
            tpf.file_key,
            MAX(tpf.item_count) as item_count,
            MAX(tpf.process_status) as process_status,
            MAX(tpf.processed_at) as processed_at,
            MIN(tpf.processed_at) as first_processed_at
        FROM transform_processed_files tpf
        INNER JOIN embedded_datasets ed ON ed.embedded_dataset_id = tpf.transform_id
        WHERE tpf.transform_type = 'dataset'
        GROUP BY ed.dataset_transform_id, ed.embedded_dataset_id, tpf.file_key
    ),
    source_chunks AS (
        SELECT COALESCE(SUM(jsonb_array_length(chunks)), 0)::BIGINT as chunk_count
        FROM dataset_items
        WHERE dataset_id = (SELECT source_dataset_id FROM dataset_transforms WHERE dataset_transform_id = $1)
    )
    SELECT
        dt.dataset_transform_id,
        COALESCE(array_length(dt.embedder_ids, 1), 0)::INTEGER as embedder_count,
        COALESCE(COUNT(DISTINCT (ub.embedded_dataset_id, ub.file_key)), 0)::BIGINT as total_batches_processed,
        COALESCE(COUNT(DISTINCT CASE WHEN ub.process_status = 'completed' THEN (ub.embedded_dataset_id, ub.file_key) END), 0)::BIGINT as successful_batches,
        COALESCE(COUNT(DISTINCT CASE WHEN ub.process_status = 'failed' THEN (ub.embedded_dataset_id, ub.file_key) END), 0)::BIGINT as failed_batches,
        COALESCE(COUNT(DISTINCT CASE WHEN ub.process_status = 'processing' THEN (ub.embedded_dataset_id, ub.file_key) END), 0)::BIGINT as processing_batches,
        COALESCE(SUM(CASE WHEN ub.process_status = 'completed' THEN ub.item_count ELSE 0 END), 0)::BIGINT as total_chunks_embedded,
        COALESCE(SUM(CASE WHEN ub.process_status = 'processing' THEN ub.item_count ELSE 0 END), 0)::BIGINT as total_chunks_processing,
        COALESCE(SUM(CASE WHEN ub.process_status = 'failed' THEN ub.item_count ELSE 0 END), 0)::BIGINT as total_chunks_failed,
        -- Multiply source chunks by embedder count to get total work across all embedders
        (SELECT chunk_count FROM source_chunks) * COALESCE(array_length(dt.embedder_ids, 1), 1) as total_chunks_to_process,
        MAX(ub.processed_at) as last_run_at,
        MIN(CASE WHEN ub.process_status = 'processing' THEN ub.first_processed_at END) as first_processing_at
    FROM dataset_transforms dt
    LEFT JOIN unique_batches ub ON ub.dataset_transform_id = dt.dataset_transform_id
    WHERE dt.dataset_transform_id = $1
    GROUP BY dt.dataset_transform_id, dt.embedder_ids
"#;

pub async fn get_dataset_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_transform_id: i32,
) -> Result<DatasetTransform> {
    let transform = sqlx::query_as::<_, DatasetTransform>(GET_DATASET_TRANSFORM_QUERY)
        .bind(owner)
        .bind(dataset_transform_id)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

pub async fn get_dataset_transforms(
    pool: &Pool<Postgres>,
    owner: &str,
) -> Result<Vec<DatasetTransform>> {
    let transforms = sqlx::query_as::<_, DatasetTransform>(GET_DATASET_TRANSFORMS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await?;
    Ok(transforms)
}

pub async fn get_dataset_transforms_for_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
) -> Result<Vec<DatasetTransform>> {
    let transforms =
        sqlx::query_as::<_, DatasetTransform>(GET_DATASET_TRANSFORMS_FOR_DATASET_QUERY)
            .bind(owner)
            .bind(dataset_id)
            .fetch_all(pool)
            .await?;
    Ok(transforms)
}

pub async fn get_active_dataset_transforms(pool: &Pool<Postgres>) -> Result<Vec<DatasetTransform>> {
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
    owner: &str,
    job_config: &serde_json::Value,
) -> Result<(DatasetTransform, Vec<EmbeddedDataset>)> {
    let mut tx = pool.begin().await.context("Failed to begin transaction")?;

    // Step 1: Create the Dataset Transform
    let transform = sqlx::query_as::<_, DatasetTransform>(CREATE_DATASET_TRANSFORM_QUERY)
        .bind(title)
        .bind(source_dataset_id)
        .bind(embedder_ids.to_vec())
        .bind(owner)
        .bind(job_config)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to create dataset transform")?;

    // Step 2: Create N Embedded Datasets (one per embedder)
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
        .bind(owner)
        .bind(dataset_transform_id)
        .bind(title)
        .bind(is_enabled)
        .bind(embedder_ids.map(|ids| ids.to_vec()))
        .bind(job_config)
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
        .bind(owner)
        .bind(dataset_transform_id)
        .execute(pool)
        .await?;
    Ok(())
}

// Statistics

pub async fn get_dataset_transform_stats(
    pool: &Pool<Postgres>,
    dataset_transform_id: i32,
) -> Result<DatasetTransformStats> {
    let stats = sqlx::query_as::<_, DatasetTransformStats>(GET_DATASET_TRANSFORM_STATS_QUERY)
        .bind(dataset_transform_id)
        .fetch_one(pool)
        .await?;
    Ok(stats)
}

// Helper functions

/// Creates an embedded dataset within a transaction
async fn create_embedded_dataset_internal(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    source_dataset_id: i32,
    embedder_id: i32,
    owner: &str,
    dataset_transform_title: &str,
) -> Result<EmbeddedDataset> {
    // Import the create function from embedded_datasets module
    use crate::storage::postgres::embedded_datasets::create_embedded_dataset_in_transaction;

    let title = format!("{dataset_transform_title}-{embedder_id}");
    let collection_name = EmbeddedDataset::generate_collection_name(0, owner); // Will be updated with actual ID

    create_embedded_dataset_in_transaction(
        tx,
        &title,
        dataset_transform_id,
        source_dataset_id,
        embedder_id,
        owner,
        &collection_name,
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
    for embedder_id in to_add {
        create_embedded_dataset_internal(
            tx,
            transform.dataset_transform_id,
            transform.source_dataset_id,
            embedder_id,
            &transform.owner,
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
    use crate::storage::postgres::embedded_datasets::get_embedded_datasets_for_transform_in_transaction;
    get_embedded_datasets_for_transform_in_transaction(executor, dataset_transform_id).await
}

async fn delete_embedded_dataset_internal(
    tx: &mut Transaction<'_, Postgres>,
    embedded_dataset_id: i32,
) -> Result<()> {
    use crate::storage::postgres::embedded_datasets::delete_embedded_dataset_in_transaction;
    delete_embedded_dataset_in_transaction(tx, embedded_dataset_id).await
}
