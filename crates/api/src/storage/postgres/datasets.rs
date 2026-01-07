use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::datasets::models::{ChunkWithMetadata, Dataset, DatasetItem};
use semantic_explorer_core::observability::record_database_query;

const GET_DATASET_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner, tags, is_public, created_at, updated_at FROM datasets
    WHERE owner = $1 AND dataset_id = $2
"#;

const GET_DATASETS_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner, tags, is_public, created_at, updated_at FROM datasets WHERE owner = $1
"#;

const CREATE_DATASET_QUERY: &str = r#"
    INSERT INTO datasets (title, details, owner, tags, is_public)
    VALUES ($1, $2, $3, $4, $5)
    RETURNING dataset_id, title, details, owner, tags, is_public, created_at, updated_at
"#;

const DELETE_DATASET_QUERY: &str = r#"
    DELETE FROM datasets WHERE owner = $1 AND dataset_id = $2
    RETURNING dataset_id, title, details, owner, tags, is_public, created_at, updated_at
"#;

const INSERT_DATASET_ITEM_QUERY: &str = r#"
    INSERT INTO dataset_items (dataset_id, title, chunks, metadata)
    VALUES ($1, $2, $3, $4)
    RETURNING item_id, dataset_id, title, chunks, metadata
"#;

const GET_DATASET_ITEMS_QUERY: &str = r#"
    SELECT item_id, dataset_id, title, chunks, metadata
    FROM dataset_items
    WHERE dataset_id = $1
    ORDER BY item_id DESC
    LIMIT $2 OFFSET $3
"#;

const COUNT_DATASET_ITEMS_QUERY: &str = r#"
    SELECT COUNT(*) as count FROM dataset_items WHERE dataset_id = $1
"#;

const UPDATE_DATASET_QUERY: &str = r#"
    UPDATE datasets
    SET title = $1, details = $2, tags = $3, is_public = $4, updated_at = NOW()
    WHERE dataset_id = $5 AND owner = $6
    RETURNING dataset_id, title, details, owner, tags, is_public, created_at, updated_at
"#;

const DELETE_DATASET_ITEM_QUERY: &str = r#"
    DELETE FROM dataset_items
    WHERE item_id = $1 AND dataset_id = $2
    RETURNING item_id, dataset_id, title, chunks, metadata
"#;

const GET_DATASET_STATS_QUERY: &str = r#"
    SELECT
        d.dataset_id,
        COUNT(di.item_id) as item_count,
        COALESCE(SUM(jsonb_array_length(di.chunks)), 0) as total_chunks
    FROM datasets d
    LEFT JOIN dataset_items di ON d.dataset_id = di.dataset_id
    WHERE d.owner = $1
    GROUP BY d.dataset_id
"#;

const GET_PUBLIC_DATASETS_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner, tags, is_public, created_at, updated_at
    FROM datasets
    WHERE is_public = TRUE
    ORDER BY created_at DESC
"#;

const GRAB_PUBLIC_DATASET_QUERY: &str = r#"
    WITH source AS (
        SELECT dataset_id, title, details, tags FROM datasets WHERE dataset_id = $1 AND is_public = TRUE
    ), new_dataset AS (
        INSERT INTO datasets (title, details, owner, tags, is_public)
        SELECT title, details, $2, tags, FALSE FROM source
        RETURNING dataset_id, title, details, owner, tags, is_public, created_at, updated_at
    )
    SELECT * FROM new_dataset
"#;

#[tracing::instrument(name = "database.get_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner, dataset_id = %dataset_id))]
pub(crate) async fn get_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
) -> Result<Dataset> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(GET_DATASET_QUERY)
        .bind(owner)
        .bind(dataset_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "datasets", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_datasets", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_datasets(pool: &Pool<Postgres>, owner: &str) -> Result<Vec<Dataset>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(GET_DATASETS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "datasets", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.create_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", title = %title, owner = %owner))]
pub(crate) async fn create_dataset(
    pool: &Pool<Postgres>,
    title: &str,
    details: Option<&str>,
    owner: &str,
    tags: &[String],
    is_public: bool,
) -> Result<Dataset> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(CREATE_DATASET_QUERY)
        .bind(title)
        .bind(details)
        .bind(owner)
        .bind(tags)
        .bind(is_public)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "datasets", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", dataset_id = %dataset_id, owner = %owner))]
pub(crate) async fn delete_dataset(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    owner: &str,
) -> Result<()> {
    let start = Instant::now();
    let result = sqlx::query(DELETE_DATASET_QUERY)
        .bind(owner)
        .bind(dataset_id)
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "datasets", duration, success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.create_dataset_item", skip(pool, metadata), fields(database.system = "postgresql", database.operation = "INSERT", dataset_id = %dataset_id, title = %title, chunk_count = chunks.len()))]
pub(crate) async fn create_dataset_item(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    title: &str,
    chunks: &[ChunkWithMetadata],
    metadata: serde_json::Value,
) -> Result<DatasetItem> {
    let chunks_json = serde_json::to_value(chunks)?;
    let item = sqlx::query_as::<_, DatasetItem>(INSERT_DATASET_ITEM_QUERY)
        .bind(dataset_id)
        .bind(title)
        .bind(&chunks_json)
        .bind(&metadata)
        .fetch_one(pool)
        .await?;
    Ok(item)
}

/// Batch insert dataset items for efficient bulk upload
/// Returns a tuple of (successfully inserted items, failed titles)
pub(crate) async fn create_dataset_items_batch(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    items: Vec<(String, Vec<ChunkWithMetadata>, serde_json::Value)>,
) -> Result<(Vec<DatasetItem>, Vec<String>)> {
    use std::time::Instant;
    let start = Instant::now();

    let mut successful = Vec::new();
    let mut failed = Vec::new();

    // Process items in batches of 1000 for efficiency
    for chunk in items.chunks(1000) {
        for (title, chunks, metadata) in chunk {
            match create_dataset_item(pool, dataset_id, title, chunks, metadata.clone()).await {
                Ok(item) => successful.push(item),
                Err(e) => {
                    failed.push(title.clone());
                    tracing::warn!("Failed to insert dataset item '{}': {}", title, e);
                }
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    let success = failed.is_empty();
    record_database_query("INSERT", "dataset_items", duration, success);

    Ok((successful, failed))
}

#[tracing::instrument(name = "database.get_dataset_items", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id, page = %page, page_size = %page_size))]
pub(crate) async fn get_dataset_items(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    page: i64,
    page_size: i64,
) -> Result<Vec<DatasetItem>> {
    let offset = page * page_size;
    let items: Vec<DatasetItem> = sqlx::query_as::<_, DatasetItem>(GET_DATASET_ITEMS_QUERY)
        .bind(dataset_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(items)
}

#[tracing::instrument(name = "database.count_dataset_items", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id))]
pub(crate) async fn count_dataset_items(pool: &Pool<Postgres>, dataset_id: i32) -> Result<i64> {
    let count: (i64,) = sqlx::query_as(COUNT_DATASET_ITEMS_QUERY)
        .bind(dataset_id)
        .fetch_one(pool)
        .await?;
    Ok(count.0)
}

#[tracing::instrument(name = "database.update_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", dataset_id = %dataset_id, owner = %owner))]
pub(crate) async fn update_dataset(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    title: &str,
    details: Option<&str>,
    owner: &str,
    tags: &[String],
    is_public: bool,
) -> Result<Dataset> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(UPDATE_DATASET_QUERY)
        .bind(title)
        .bind(details)
        .bind(tags)
        .bind(is_public)
        .bind(dataset_id)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "datasets", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_dataset_item", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", item_id = %item_id, dataset_id = %dataset_id))]
pub(crate) async fn delete_dataset_item(
    pool: &Pool<Postgres>,
    item_id: i32,
    dataset_id: i32,
) -> Result<DatasetItem> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, DatasetItem>(DELETE_DATASET_ITEM_QUERY)
        .bind(item_id)
        .bind(dataset_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "dataset_items", duration, success);

    Ok(result?)
}

#[derive(Debug)]
pub(crate) struct DatasetStats {
    pub(crate) dataset_id: i32,
    pub(crate) item_count: i64,
    pub(crate) total_chunks: i64,
}

impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for DatasetStats {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(DatasetStats {
            dataset_id: row.try_get("dataset_id")?,
            item_count: row.try_get("item_count")?,
            total_chunks: row.try_get("total_chunks")?,
        })
    }
}

#[tracing::instrument(name = "database.get_dataset_stats", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_dataset_stats(
    pool: &Pool<Postgres>,
    owner: &str,
) -> Result<Vec<DatasetStats>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, DatasetStats>(GET_DATASET_STATS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "datasets_stats", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_public_datasets", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_public_datasets(pool: &Pool<Postgres>) -> Result<Vec<Dataset>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(GET_PUBLIC_DATASETS_QUERY)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "datasets", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.grab_public_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", owner = %owner, dataset_id = %dataset_id))]
pub(crate) async fn grab_public_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    dataset_id: i32,
) -> Result<Dataset> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(GRAB_PUBLIC_DATASET_QUERY)
        .bind(dataset_id)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "datasets", duration, success);

    Ok(result?)
}
