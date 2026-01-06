use anyhow::Result;
use sqlx::{Pool, Postgres, Transaction};

use crate::embedded_datasets::{
    EmbeddedDataset, EmbeddedDatasetProcessedBatch, EmbeddedDatasetStats,
    EmbeddedDatasetWithDetails,
};

// Query constants
const GET_EMBEDDED_DATASET_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner, collection_name, created_at, updated_at
    FROM embedded_datasets
    WHERE owner = $1 AND embedded_dataset_id = $2
"#;

const GET_EMBEDDED_DATASETS_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner, collection_name, created_at, updated_at
    FROM embedded_datasets
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

const GET_EMBEDDED_DATASETS_FOR_DATASET_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner, collection_name, created_at, updated_at
    FROM embedded_datasets
    WHERE owner = $1 AND source_dataset_id = $2
    ORDER BY created_at DESC
"#;

const GET_EMBEDDED_DATASETS_FOR_TRANSFORM_QUERY: &str = r#"
    SELECT embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
           owner, collection_name, created_at, updated_at
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
        ed.owner,
        ed.collection_name,
        ed.created_at,
        ed.updated_at
    FROM embedded_datasets ed
    INNER JOIN datasets d ON d.dataset_id = ed.source_dataset_id
    INNER JOIN embedders e ON e.embedder_id = ed.embedder_id
    WHERE ed.owner = $1 AND ed.embedded_dataset_id = $2
"#;

const CREATE_EMBEDDED_DATASET_QUERY: &str = r#"
    INSERT INTO embedded_datasets (title, dataset_transform_id, source_dataset_id, embedder_id, owner, collection_name)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner, collection_name, created_at, updated_at
"#;

const UPDATE_EMBEDDED_DATASET_COLLECTION_NAME_QUERY: &str = r#"
    UPDATE embedded_datasets
    SET collection_name = $2,
        updated_at = NOW()
    WHERE embedded_dataset_id = $1
    RETURNING embedded_dataset_id, title, dataset_transform_id, source_dataset_id, embedder_id,
              owner, collection_name, created_at, updated_at
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
        COALESCE(SUM(tpf.item_count) FILTER (WHERE tpf.process_status = 'completed'), 0)::BIGINT as total_chunks_embedded,
        COALESCE(SUM(tpf.item_count) FILTER (WHERE tpf.process_status = 'failed'), 0)::BIGINT as total_chunks_failed
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

// Public CRUD operations

pub async fn get_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<EmbeddedDataset> {
    let embedded_dataset = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASET_QUERY)
        .bind(owner)
        .bind(embedded_dataset_id)
        .fetch_one(pool)
        .await?;
    Ok(embedded_dataset)
}

pub async fn get_embedded_datasets(
    pool: &Pool<Postgres>,
    owner: &str,
) -> Result<Vec<EmbeddedDataset>> {
    let embedded_datasets = sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await?;
    Ok(embedded_datasets)
}

pub async fn get_embedded_datasets_for_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
) -> Result<Vec<EmbeddedDataset>> {
    let embedded_datasets =
        sqlx::query_as::<_, EmbeddedDataset>(GET_EMBEDDED_DATASETS_FOR_DATASET_QUERY)
            .bind(owner)
            .bind(dataset_id)
            .fetch_all(pool)
            .await?;
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
    let embedded_dataset =
        sqlx::query_as::<_, EmbeddedDatasetWithDetails>(GET_EMBEDDED_DATASET_WITH_DETAILS_QUERY)
            .bind(owner)
            .bind(embedded_dataset_id)
            .fetch_one(pool)
            .await?;
    Ok(embedded_dataset)
}

pub async fn delete_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<()> {
    // First verify ownership
    get_embedded_dataset(pool, owner, embedded_dataset_id).await?;

    // Then delete (cascades to processed files)
    sqlx::query(DELETE_EMBEDDED_DATASET_QUERY)
        .bind(embedded_dataset_id)
        .execute(pool)
        .await?;
    Ok(())
}

// Statistics and processed batches

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

pub async fn record_processed_batch(
    pool: &Pool<Postgres>,
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
        .execute(pool)
        .await?;
    Ok(())
}

// Transaction-aware helper functions (used by dataset_transforms module)

pub async fn create_embedded_dataset_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    title: &str,
    dataset_transform_id: i32,
    source_dataset_id: i32,
    embedder_id: i32,
    owner: &str,
    collection_name: &str,
) -> Result<EmbeddedDataset> {
    let mut embedded_dataset = sqlx::query_as::<_, EmbeddedDataset>(CREATE_EMBEDDED_DATASET_QUERY)
        .bind(title)
        .bind(dataset_transform_id)
        .bind(source_dataset_id)
        .bind(embedder_id)
        .bind(owner)
        .bind(collection_name)
        .fetch_one(&mut **tx)
        .await?;

    // Update collection name with actual ID
    let actual_collection_name =
        EmbeddedDataset::generate_collection_name(embedded_dataset.embedded_dataset_id, owner);
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
