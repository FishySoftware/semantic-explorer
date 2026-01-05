use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::datasets::models::{ChunkWithMetadata, Dataset, DatasetItem};
use semantic_explorer_core::observability::record_database_query;

const GET_DATASET_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner, tags, created_at, updated_at FROM datasets
    WHERE owner = $1 AND dataset_id = $2
"#;

const GET_DATASETS_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner, tags, created_at, updated_at FROM datasets WHERE owner = $1
"#;

const CREATE_DATASET_QUERY: &str = r#"
    INSERT INTO datasets (title, details, owner, tags)
    VALUES ($1, $2, $3, $4)
    RETURNING dataset_id, title, details, owner, tags, created_at, updated_at
"#;

const DELETE_DATASET_QUERY: &str = r#"
    DELETE FROM datasets WHERE owner = $1 AND dataset_id = $2
    RETURNING dataset_id, title, details, owner, tags, created_at, updated_at
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
    SET title = $1, details = $2, tags = $3, updated_at = NOW()
    WHERE dataset_id = $4 AND owner = $5
    RETURNING dataset_id, title, details, owner, tags, created_at, updated_at
"#;

const DELETE_DATASET_ITEM_QUERY: &str = r#"
    DELETE FROM dataset_items
    WHERE item_id = $1 AND dataset_id = $2
    RETURNING item_id, dataset_id, title, chunks, metadata
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
) -> Result<Dataset> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(CREATE_DATASET_QUERY)
        .bind(title)
        .bind(details)
        .bind(owner)
        .bind(tags)
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
) -> Result<Dataset> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Dataset>(UPDATE_DATASET_QUERY)
        .bind(title)
        .bind(details)
        .bind(tags)
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
