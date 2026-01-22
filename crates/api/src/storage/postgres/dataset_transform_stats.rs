use crate::transforms::dataset::models::DatasetTransformStats;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres, Transaction};

const INIT_STATS_QUERY: &str = r#"
    INSERT INTO dataset_transform_stats (
        dataset_transform_id,
        total_chunks_to_process,
        created_at,
        updated_at
    )
    VALUES ($1, $2, NOW(), NOW())
    ON CONFLICT (dataset_transform_id) DO NOTHING
    RETURNING dataset_transform_id
"#;

const INCREMENT_SUCCESS_QUERY: &str = r#"
    UPDATE dataset_transform_stats
    SET 
        total_chunks_embedded = total_chunks_embedded + $2,
        successful_batches = successful_batches + 1,
        last_processed_at = $3,
        updated_at = NOW()
    WHERE dataset_transform_id = $1
"#;

const INCREMENT_FAILED_QUERY: &str = r#"
    UPDATE dataset_transform_stats
    SET 
        total_chunks_failed = total_chunks_failed + $2,
        failed_batches = failed_batches + 1,
        last_processed_at = $3,
        updated_at = NOW()
    WHERE dataset_transform_id = $1
"#;

const INCREMENT_PROCESSING_QUERY: &str = r#"
    UPDATE dataset_transform_stats
    SET 
        total_chunks_processing = total_chunks_processing + $2,
        processing_batches = processing_batches + 1,
        first_processing_at = COALESCE(first_processing_at, $3),
        updated_at = NOW()
    WHERE dataset_transform_id = $1
"#;

const DECREMENT_PROCESSING_QUERY: &str = r#"
    UPDATE dataset_transform_stats
    SET 
        total_chunks_processing = GREATEST(0, total_chunks_processing - $2),
        processing_batches = GREATEST(0, processing_batches - 1),
        updated_at = NOW()
    WHERE dataset_transform_id = $1
"#;

const GET_STATS_QUERY: &str = r#"
    SELECT 
        dt.dataset_transform_id,
        COALESCE(array_length(dt.embedder_ids, 1), 0)::INTEGER as embedder_count,
        COALESCE(dts.successful_batches, 0)::BIGINT as successful_batches,
        COALESCE(dts.failed_batches, 0)::BIGINT as failed_batches,
        COALESCE(dts.processing_batches, 0)::BIGINT as processing_batches,
        (COALESCE(dts.successful_batches, 0) + COALESCE(dts.failed_batches, 0) + COALESCE(dts.processing_batches, 0))::BIGINT as total_batches_processed,
        COALESCE(dts.total_chunks_embedded, 0)::BIGINT as total_chunks_embedded,
        COALESCE(dts.total_chunks_processing, 0)::BIGINT as total_chunks_processing,
        COALESCE(dts.total_chunks_failed, 0)::BIGINT as total_chunks_failed,
        COALESCE(dts.total_chunks_to_process, 0)::BIGINT as total_chunks_to_process,
        dts.last_processed_at as last_run_at,
        dts.first_processing_at
    FROM dataset_transforms dt
    LEFT JOIN dataset_transform_stats dts ON dts.dataset_transform_id = dt.dataset_transform_id
    WHERE dt.dataset_transform_id = $1
"#;

const REFRESH_TOTAL_CHUNKS_QUERY: &str = r#"
    UPDATE dataset_transform_stats dts
    SET 
        total_chunks_to_process = d.total_chunks * COALESCE(array_length(dt.embedder_ids, 1), 1),
        updated_at = NOW()
    FROM dataset_transforms dt
    INNER JOIN datasets d ON d.dataset_id = dt.source_dataset_id
    WHERE dts.dataset_transform_id = dt.dataset_transform_id
      AND dts.dataset_transform_id = $1
    RETURNING dts.total_chunks_to_process
"#;

const GET_BATCH_STATS_QUERY: &str = r#"
    SELECT 
        dt.dataset_transform_id,
        COALESCE(array_length(dt.embedder_ids, 1), 0)::INTEGER as embedder_count,
        COALESCE(dts.successful_batches, 0)::BIGINT as successful_batches,
        COALESCE(dts.failed_batches, 0)::BIGINT as failed_batches,
        COALESCE(dts.processing_batches, 0)::BIGINT as processing_batches,
        (COALESCE(dts.successful_batches, 0) + COALESCE(dts.failed_batches, 0) + COALESCE(dts.processing_batches, 0))::BIGINT as total_batches_processed,
        COALESCE(dts.total_chunks_embedded, 0)::BIGINT as total_chunks_embedded,
        COALESCE(dts.total_chunks_processing, 0)::BIGINT as total_chunks_processing,
        COALESCE(dts.total_chunks_failed, 0)::BIGINT as total_chunks_failed,
        COALESCE(dts.total_chunks_to_process, 0)::BIGINT as total_chunks_to_process,
        dts.last_processed_at as last_run_at,
        dts.first_processing_at
    FROM dataset_transforms dt
    LEFT JOIN dataset_transform_stats dts ON dts.dataset_transform_id = dt.dataset_transform_id
    WHERE dt.dataset_transform_id = ANY($1)
    ORDER BY dt.dataset_transform_id
"#;

pub async fn initialize_stats(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    total_chunks_to_process: i64,
) -> Result<()> {
    sqlx::query(INIT_STATS_QUERY)
        .bind(dataset_transform_id)
        .bind(total_chunks_to_process)
        .fetch_optional(&mut **tx)
        .await
        .context("Failed to initialize dataset transform stats")?;
    Ok(())
}

pub async fn increment_successful_batch(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    chunk_count: i64,
    processed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(INCREMENT_SUCCESS_QUERY)
        .bind(dataset_transform_id)
        .bind(chunk_count)
        .bind(processed_at)
        .execute(&mut **tx)
        .await
        .context("Failed to increment successful batch stats")?;
    Ok(())
}

pub async fn increment_failed_batch(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    chunk_count: i64,
    processed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(INCREMENT_FAILED_QUERY)
        .bind(dataset_transform_id)
        .bind(chunk_count)
        .bind(processed_at)
        .execute(&mut **tx)
        .await
        .context("Failed to increment failed batch stats")?;
    Ok(())
}

pub async fn increment_processing_batch(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    chunk_count: i64,
    started_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(INCREMENT_PROCESSING_QUERY)
        .bind(dataset_transform_id)
        .bind(chunk_count)
        .bind(started_at)
        .execute(&mut **tx)
        .await
        .context("Failed to increment processing batch stats")?;
    Ok(())
}

pub async fn decrement_processing_batch(
    tx: &mut Transaction<'_, Postgres>,
    dataset_transform_id: i32,
    chunk_count: i64,
) -> Result<()> {
    sqlx::query(DECREMENT_PROCESSING_QUERY)
        .bind(dataset_transform_id)
        .bind(chunk_count)
        .execute(&mut **tx)
        .await
        .context("Failed to decrement processing batch stats")?;
    Ok(())
}

/// Get stats for a single dataset transform with ownership verification
pub async fn get_stats(
    pool: &Pool<Postgres>,
    owner_id: &str,
    dataset_transform_id: i32,
) -> Result<Option<DatasetTransformStats>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let stats = sqlx::query_as::<_, DatasetTransformStats>(GET_STATS_QUERY)
        .bind(dataset_transform_id)
        .fetch_optional(&mut *tx)
        .await
        .with_context(|| {
            format!(
                "Failed to get dataset transform stats for transform {} with owner {}",
                dataset_transform_id, owner_id
            )
        })?;

    tx.commit().await?;
    Ok(stats)
}

pub async fn get_batch_stats(
    pool: &Pool<Postgres>,
    owner_id: &str,
    dataset_transform_ids: &[i32],
) -> Result<Vec<DatasetTransformStats>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let stats = sqlx::query_as::<_, DatasetTransformStats>(GET_BATCH_STATS_QUERY)
        .bind(dataset_transform_ids)
        .fetch_all(&mut *tx)
        .await
        .context("Failed to get batch dataset transform stats")?;

    tx.commit().await?;
    Ok(stats)
}

pub async fn refresh_total_chunks(
    pool: &Pool<Postgres>,
    owner_id: &str,
    dataset_transform_id: i32,
) -> Result<Option<i64>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let result = sqlx::query_scalar::<_, i64>(REFRESH_TOTAL_CHUNKS_QUERY)
        .bind(dataset_transform_id)
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to refresh total chunks to process")?;

    tx.commit().await?;
    Ok(result)
}
