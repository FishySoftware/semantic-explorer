use anyhow::Result;
use sqlx::{
    Pool, Postgres,
    types::chrono::{DateTime, Utc},
};

use crate::datasets::models::{ChunkWithMetadata, Dataset, DatasetItem};

const GET_DATASET_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at FROM datasets
    WHERE dataset_id = $1 AND owner_id = $2
"#;

const GET_DATASETS_PAGINATED_QUERY: &str = r#"
    SELECT d.dataset_id, d.title, d.details, d.owner_id, d.owner_display_name, d.tags, d.is_public, d.item_count, d.total_chunks, d.created_at, d.updated_at,
        COUNT(*) OVER() AS total_count
    FROM datasets d
    WHERE d.owner_id = $1
    ORDER BY d.created_at DESC
    LIMIT $2 OFFSET $3
"#;

const GET_DATASETS_PAGINATED_SEARCH_QUERY: &str = r#"
    SELECT d.dataset_id, d.title, d.details, d.owner_id, d.owner_display_name, d.tags, d.is_public, d.item_count, d.total_chunks, d.created_at, d.updated_at,
        COUNT(*) OVER() AS total_count
    FROM datasets d
    WHERE d.owner_id = $3 AND (d.title ILIKE $4 OR d.details ILIKE $4 OR $5 = ANY(d.tags))
    ORDER BY d.created_at DESC
    LIMIT $1 OFFSET $2
"#;

const CREATE_DATASET_QUERY: &str = r#"
    INSERT INTO datasets (title, details, owner_id, owner_display_name, tags, is_public)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING dataset_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at
"#;

const DELETE_DATASET_QUERY: &str = r#"
    DELETE FROM datasets WHERE dataset_id = $1 AND owner_id = $2
    RETURNING dataset_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at
"#;

const INSERT_DATASET_ITEM_QUERY: &str = r#"
    INSERT INTO dataset_items (dataset_id, title, chunks, metadata)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (dataset_id, title)
    DO UPDATE SET
        chunks = EXCLUDED.chunks,
        metadata = EXCLUDED.metadata,
        updated_at = NOW()
    RETURNING item_id, dataset_id, title, chunks, metadata, created_at, updated_at
"#;

const GET_DATASET_ITEMS_QUERY: &str = r#"
    SELECT item_id, dataset_id, title, chunks, metadata, created_at, COALESCE(updated_at, created_at) as updated_at
    FROM dataset_items
    WHERE dataset_id = $1
    ORDER BY item_id DESC
    LIMIT $2 OFFSET $3
"#;

const GET_DATASET_ITEMS_SUMMARY_QUERY: &str = r#"
    SELECT item_id, dataset_id, title, jsonb_array_length(chunks) as chunk_count, metadata, created_at, COALESCE(updated_at, created_at) as updated_at
    FROM dataset_items
    WHERE dataset_id = $1
    ORDER BY item_id DESC
    LIMIT $2 OFFSET $3
"#;

const GET_DATASET_ITEMS_SUMMARY_WITH_SEARCH_QUERY: &str = r#"
    SELECT item_id, dataset_id, title, jsonb_array_length(chunks) as chunk_count, metadata, created_at, COALESCE(updated_at, created_at) as updated_at
    FROM dataset_items
    WHERE dataset_id = $1 AND title ILIKE $4
    ORDER BY item_id DESC
    LIMIT $2 OFFSET $3
"#;

const GET_DATASET_ITEM_CHUNKS_QUERY: &str = r#"
    SELECT item_id, dataset_id, title, chunks, metadata
    FROM dataset_items
    WHERE dataset_id = $1 AND item_id = $2
"#;

const COUNT_DATASET_ITEMS_QUERY: &str = r#"
    SELECT COUNT(*) as count FROM dataset_items WHERE dataset_id = $1
"#;

const COUNT_DATASET_ITEMS_WITH_SEARCH_QUERY: &str = r#"
    SELECT COUNT(*) as count FROM dataset_items WHERE dataset_id = $1 AND title ILIKE $2
"#;

/// Scanner query: fetches dataset items with item_id > last_processed_item_id.
/// Uses item_id (PostgreSQL SERIAL, monotonically increasing) as the watermark
/// instead of timestamps. This eliminates the MVCC race condition where concurrent
/// transactions insert items with created_at timestamps BEFORE the scanner's
/// query_start_time, causing them to be permanently skipped.
const GET_DATASET_ITEMS_FOR_SCANNER_QUERY: &str = r#"
    SELECT item_id, dataset_id, title, chunks, metadata, created_at, COALESCE(updated_at, created_at) as updated_at
    FROM dataset_items
    WHERE dataset_id = $1 AND item_id > $2
    ORDER BY item_id ASC
    LIMIT $3
"#;

/// Get the dataset's version (updated_at timestamp) for efficient stats refresh (#4)
const GET_DATASET_VERSION_QUERY: &str = r#"
    SELECT COALESCE(updated_at, created_at)
    FROM datasets
    WHERE dataset_id = $1
"#;

const UPDATE_DATASET_QUERY: &str = r#"
    UPDATE datasets
    SET title = $1, details = $2, tags = $3, is_public = $4, updated_at = NOW()
    WHERE dataset_id = $5 AND owner_id = $6
    RETURNING dataset_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at
"#;

const DELETE_DATASET_ITEM_QUERY: &str = r#"
    DELETE FROM dataset_items
    WHERE item_id = $1 AND dataset_id = $2
    RETURNING item_id, dataset_id, title, chunks, metadata, created_at, COALESCE(updated_at, created_at) as updated_at
"#;

const GET_PUBLIC_DATASETS_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at
    FROM datasets
    WHERE is_public = TRUE
    ORDER BY created_at DESC
    LIMIT $1 OFFSET $2
"#;

const GET_RECENT_PUBLIC_DATASETS_QUERY: &str = r#"
    SELECT dataset_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at
    FROM datasets
    WHERE is_public = TRUE
    ORDER BY updated_at DESC
    LIMIT $1
"#;

const GRAB_PUBLIC_DATASET_QUERY: &str = r#"
    WITH source AS (
        SELECT dataset_id, title, details, tags FROM datasets WHERE dataset_id = $1 AND is_public = TRUE
    ), new_dataset AS (
        INSERT INTO datasets (title, details, owner_id, owner_display_name, tags, is_public)
        SELECT title || '-grabbed', details, $2, $3, tags, FALSE FROM source
        RETURNING dataset_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at
    )
    SELECT * FROM new_dataset
"#;

/// Copy all items from one dataset to another in a single INSERT...SELECT statement.
/// This avoids N+1 inserts and N trigger firings in separate transactions.
const COPY_DATASET_ITEMS_QUERY: &str = r#"
    INSERT INTO dataset_items (dataset_id, title, chunks, metadata)
    SELECT $1, title, chunks, metadata
    FROM dataset_items
    WHERE dataset_id = $2
"#;

const CREATE_DATASET_ITEMS_BATCH: &str = r#"
    INSERT INTO dataset_items (dataset_id, title, chunks, metadata)
    SELECT $1, unnest($2::text[]), unnest($3::jsonb[]), unnest($4::jsonb[])
    ON CONFLICT (dataset_id, title)
    DO UPDATE SET
        chunks = EXCLUDED.chunks,
        metadata = EXCLUDED.metadata,
        updated_at = NOW()
    RETURNING item_id, dataset_id, title, chunks, metadata, created_at, updated_at
"#;

#[tracing::instrument(name = "database.get_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, dataset_id = %dataset_id))]
pub(crate) async fn get_dataset(
    pool: &Pool<Postgres>,
    owner_id: &str,
    dataset_id: i32,
) -> Result<Dataset> {
    let result = sqlx::query_as::<_, Dataset>(GET_DATASET_QUERY)
        .bind(dataset_id)
        .bind(owner_id)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct DatasetWithStatsRow {
    pub(crate) dataset_id: i32,
    pub(crate) title: String,
    pub(crate) details: Option<String>,
    pub(crate) owner_id: String,
    pub(crate) owner_display_name: String,
    pub(crate) tags: Vec<String>,
    pub(crate) is_public: bool,
    pub(crate) item_count: i32,
    pub(crate) total_chunks: i64,
    pub(crate) created_at: Option<DateTime<Utc>>,
    pub(crate) updated_at: Option<DateTime<Utc>>,
    pub(crate) total_count: i64,
}

#[derive(Debug)]
pub(crate) struct PaginatedDatasetsResult {
    pub(crate) items: Vec<DatasetWithStatsRow>,
    pub(crate) total_count: i64,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

#[tracing::instrument(name = "database.get_datasets_paginated", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, limit = %limit, offset = %offset))]
pub(crate) async fn get_datasets_paginated(
    pool: &Pool<Postgres>,
    owner_id: &str,
    limit: i64,
    offset: i64,
) -> Result<PaginatedDatasetsResult> {
    let result = sqlx::query_as::<_, DatasetWithStatsRow>(GET_DATASETS_PAGINATED_QUERY)
        .bind(owner_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    let items = result?;
    let total_count = items.first().map_or(0, |r| r.total_count);
    Ok(PaginatedDatasetsResult {
        items,
        total_count,
        limit,
        offset,
    })
}

#[tracing::instrument(name = "database.get_datasets_paginated_search", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, limit = %limit, offset = %offset))]
pub(crate) async fn get_datasets_paginated_search(
    pool: &Pool<Postgres>,
    owner_id: &str,
    search_query: &str,
    limit: i64,
    offset: i64,
) -> Result<PaginatedDatasetsResult> {
    let search_pattern = format!("%{}%", search_query);

    let result = sqlx::query_as::<_, DatasetWithStatsRow>(GET_DATASETS_PAGINATED_SEARCH_QUERY)
        .bind(limit)
        .bind(offset)
        .bind(owner_id)
        .bind(&search_pattern)
        .bind(search_query)
        .fetch_all(pool)
        .await;

    let items = result?;
    let total_count = items.first().map_or(0, |r| r.total_count);
    Ok(PaginatedDatasetsResult {
        items,
        total_count,
        limit,
        offset,
    })
}

#[tracing::instrument(name = "database.create_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", title = %title, owner_id = %owner_id))]
pub(crate) async fn create_dataset(
    pool: &Pool<Postgres>,
    title: &str,
    details: Option<&str>,
    owner_id: &str,
    owner_display_name: &str,
    tags: &[String],
    is_public: bool,
) -> Result<Dataset> {
    let result = sqlx::query_as::<_, Dataset>(CREATE_DATASET_QUERY)
        .bind(title)
        .bind(details)
        .bind(owner_id)
        .bind(owner_display_name)
        .bind(tags)
        .bind(is_public)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", dataset_id = %dataset_id, owner_id = %owner_id))]
pub(crate) async fn delete_dataset(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    owner_id: &str,
) -> Result<()> {
    let result = sqlx::query(DELETE_DATASET_QUERY)
        .bind(dataset_id)
        .bind(owner_id)
        .execute(pool)
        .await;

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.create_dataset_item", skip(pool, metadata, chunks), fields(database.system = "postgresql", database.operation = "INSERT", dataset_id = %dataset_id, title = %title, chunk_count = chunks.len(), owner_id = %owner_id))]
pub(crate) async fn create_dataset_item(
    pool: &Pool<Postgres>,
    owner_id: &str,
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

/// Batch insert dataset items for efficient bulk upload using a single multi-row INSERT
/// Returns a tuple of (successfully inserted items, failed titles)
pub(crate) async fn create_dataset_items_batch(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    items: Vec<(String, Vec<ChunkWithMetadata>, serde_json::Value)>,
) -> Result<(Vec<DatasetItem>, Vec<String>)> {
    if items.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let mut tx = pool.begin().await?;

    let mut successful = Vec::new();
    let mut failed = Vec::new();

    // Process in chunks of 500 items to avoid parameter limits
    const BATCH_SIZE: usize = 500;

    for chunk in items.chunks(BATCH_SIZE) {
        // Prepare arrays for UNNEST
        let mut titles: Vec<String> = Vec::with_capacity(chunk.len());
        let mut chunks_array: Vec<serde_json::Value> = Vec::with_capacity(chunk.len());
        let mut metadata_array: Vec<serde_json::Value> = Vec::with_capacity(chunk.len());

        for (title, chunks, metadata) in chunk {
            match serde_json::to_value(chunks) {
                Ok(chunks_json) => {
                    titles.push(title.clone());
                    chunks_array.push(chunks_json);
                    metadata_array.push(metadata.clone());
                }
                Err(e) => {
                    failed.push(title.clone());
                    tracing::warn!("Failed to serialize chunks for '{}': {}", title, e);
                }
            }
        }

        if titles.is_empty() {
            continue;
        }

        match sqlx::query_as::<_, DatasetItem>(CREATE_DATASET_ITEMS_BATCH)
            .bind(dataset_id)
            .bind(&titles)
            .bind(&chunks_array)
            .bind(&metadata_array)
            .fetch_all(&mut *tx)
            .await
        {
            Ok(items) => {
                successful.extend(items);
            }
            Err(e) => {
                failed.extend(titles.clone());
                tracing::warn!("Failed to insert dataset items: {}", e);
            }
        }
    }

    tx.commit().await?;

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
        .bind(page_size as i32)
        .bind(offset as i32)
        .fetch_all(pool)
        .await?;
    Ok(items)
}

#[tracing::instrument(name = "database.get_dataset_items_summary", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id, page = %page, page_size = %page_size))]
pub(crate) async fn get_dataset_items_summary(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    page: i64,
    page_size: i64,
) -> Result<Vec<crate::datasets::models::DatasetItemSummary>> {
    use crate::datasets::models::DatasetItemSummary;
    let offset = page * page_size;
    let items: Vec<DatasetItemSummary> =
        sqlx::query_as::<_, DatasetItemSummary>(GET_DATASET_ITEMS_SUMMARY_QUERY)
            .bind(dataset_id)
            .bind(page_size as i32)
            .bind(offset as i32)
            .fetch_all(pool)
            .await?;
    Ok(items)
}

#[tracing::instrument(name = "database.get_dataset_items_summary_with_search", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id, page = %page, page_size = %page_size))]
pub(crate) async fn get_dataset_items_summary_with_search(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    page: i64,
    page_size: i64,
    search_query: &str,
) -> Result<Vec<crate::datasets::models::DatasetItemSummary>> {
    use crate::datasets::models::DatasetItemSummary;
    let offset = page * page_size;
    let search_pattern = format!("%{}%", search_query);
    let items: Vec<DatasetItemSummary> =
        sqlx::query_as::<_, DatasetItemSummary>(GET_DATASET_ITEMS_SUMMARY_WITH_SEARCH_QUERY)
            .bind(dataset_id)
            .bind(page_size as i32)
            .bind(offset as i32)
            .bind(&search_pattern)
            .fetch_all(pool)
            .await?;
    Ok(items)
}

#[tracing::instrument(name = "database.get_dataset_item_chunks", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id, item_id = %item_id))]
pub(crate) async fn get_dataset_item_chunks(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    item_id: i32,
) -> Result<Option<crate::datasets::models::DatasetItemChunks>> {
    use crate::datasets::models::{ChunkWithMetadata, DatasetItemChunks};

    let row = sqlx::query(GET_DATASET_ITEM_CHUNKS_QUERY)
        .bind(dataset_id)
        .bind(item_id)
        .fetch_optional(pool)
        .await?;

    if let Some(row) = row {
        use sqlx::Row;

        let chunks_json: serde_json::Value = row.try_get("chunks")?;
        let chunks: Vec<ChunkWithMetadata> = serde_json::from_value(chunks_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse chunks: {}", e))?;

        Ok(Some(DatasetItemChunks {
            item_id: row.try_get("item_id")?,
            dataset_id: row.try_get("dataset_id")?,
            title: row.try_get("title")?,
            chunks,
            metadata: row.try_get("metadata")?,
        }))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(name = "database.count_dataset_items", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id))]
pub(crate) async fn count_dataset_items(pool: &Pool<Postgres>, dataset_id: i32) -> Result<i64> {
    let count: (i64,) = sqlx::query_as(COUNT_DATASET_ITEMS_QUERY)
        .bind(dataset_id)
        .fetch_one(pool)
        .await?;
    Ok(count.0)
}

#[tracing::instrument(name = "database.count_dataset_items_with_search", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id))]
pub(crate) async fn count_dataset_items_with_search(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    search_query: &str,
) -> Result<i64> {
    let search_pattern = format!("%{}%", search_query);
    let count: (i64,) = sqlx::query_as(COUNT_DATASET_ITEMS_WITH_SEARCH_QUERY)
        .bind(dataset_id)
        .bind(&search_pattern)
        .fetch_one(pool)
        .await?;
    Ok(count.0)
}

/// Fetch dataset items for the scanner using item_id-based watermark.
/// Uses `item_id > last_processed_item_id` to find new items, which is safe under
/// concurrent inserts because PostgreSQL SERIAL IDs are monotonically increasing.
/// Pass `last_processed_item_id = 0` for the initial scan (fetches all items).
#[tracing::instrument(name = "database.get_dataset_items_for_scanner", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", dataset_id = %dataset_id))]
pub(crate) async fn get_dataset_items_for_scanner(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    last_processed_item_id: i32,
    limit: i64,
) -> Result<Vec<DatasetItem>> {
    let items = sqlx::query_as::<_, DatasetItem>(GET_DATASET_ITEMS_FOR_SCANNER_QUERY)
        .bind(dataset_id)
        .bind(last_processed_item_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(items)
}

/// Get the dataset's version (updated_at timestamp) for efficient stats refresh (#4)
/// Returns None if the dataset doesn't exist
pub(crate) async fn get_dataset_version(
    pool: &Pool<Postgres>,
    dataset_id: i32,
) -> Result<Option<DateTime<Utc>>> {
    let version = sqlx::query_scalar::<_, DateTime<Utc>>(GET_DATASET_VERSION_QUERY)
        .bind(dataset_id)
        .fetch_optional(pool)
        .await?;
    Ok(version)
}

/// Get the source dataset's total_chunks count.
/// Used by the detailed-stats endpoint to provide the expected total per embedded dataset.
pub(crate) async fn get_dataset_total_chunks(
    pool: &Pool<Postgres>,
    dataset_id: i32,
) -> Result<i64> {
    let total_chunks =
        sqlx::query_scalar::<_, i64>("SELECT total_chunks FROM datasets WHERE dataset_id = $1")
            .bind(dataset_id)
            .fetch_one(pool)
            .await?;
    Ok(total_chunks)
}

#[tracing::instrument(name = "database.update_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", dataset_id = %dataset_id, owner_id = %owner_id))]
pub(crate) async fn update_dataset(
    pool: &Pool<Postgres>,
    dataset_id: i32,
    title: &str,
    details: Option<&str>,
    owner_id: &str,
    tags: &[String],
    is_public: bool,
) -> Result<Dataset> {
    let result = sqlx::query_as::<_, Dataset>(UPDATE_DATASET_QUERY)
        .bind(title)
        .bind(details)
        .bind(tags)
        .bind(is_public)
        .bind(dataset_id)
        .bind(owner_id)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_dataset_item", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", item_id = %item_id, dataset_id = %dataset_id))]
pub(crate) async fn delete_dataset_item(
    pool: &Pool<Postgres>,
    item_id: i32,
    dataset_id: i32,
) -> Result<DatasetItem> {
    let result = sqlx::query_as::<_, DatasetItem>(DELETE_DATASET_ITEM_QUERY)
        .bind(item_id)
        .bind(dataset_id)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.get_public_datasets", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_public_datasets(
    pool: &Pool<Postgres>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Dataset>> {
    let result = sqlx::query_as::<_, Dataset>(GET_PUBLIC_DATASETS_QUERY)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.get_recent_public_datasets", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_recent_public_datasets(
    pool: &Pool<Postgres>,
    limit: i32,
) -> Result<Vec<Dataset>> {
    let result = sqlx::query_as::<_, Dataset>(GET_RECENT_PUBLIC_DATASETS_QUERY)
        .bind(limit)
        .fetch_all(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.grab_public_dataset", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %owner_id, dataset_id = %dataset_id))]
pub(crate) async fn grab_public_dataset(
    pool: &Pool<Postgres>,
    owner_id: &str,
    owner_display_name: &str,
    dataset_id: i32,
) -> Result<Dataset> {
    let mut tx = pool.begin().await?;

    let result = sqlx::query_as::<_, Dataset>(GRAB_PUBLIC_DATASET_QUERY)
        .bind(dataset_id)
        .bind(owner_id)
        .bind(owner_display_name)
        .fetch_one(&mut *tx)
        .await;

    let new_dataset = result?;

    // Copy all items in a single INSERT...SELECT (avoids N+1 inserts and N trigger firings)
    let copy_result = sqlx::query(COPY_DATASET_ITEMS_QUERY)
        .bind(new_dataset.dataset_id)
        .bind(dataset_id)
        .execute(&mut *tx)
        .await;

    match copy_result {
        Ok(result) => {
            tracing::info!(
                source_dataset_id = dataset_id,
                new_dataset_id = new_dataset.dataset_id,
                rows_copied = result.rows_affected(),
                "Copied dataset items via INSERT...SELECT"
            );
        }
        Err(e) => {
            tracing::error!(
                source_dataset_id = dataset_id,
                new_dataset_id = new_dataset.dataset_id,
                error = %e,
                "Failed to copy dataset items for grabbed dataset"
            );
            // Don't fail the whole operation if item copy fails
        }
    }

    tx.commit().await?;
    Ok(new_dataset)
}
