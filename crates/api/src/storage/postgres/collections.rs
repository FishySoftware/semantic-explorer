use anyhow::Result;
use aws_sdk_s3::Client;
use sqlx::{Pool, Postgres};

use crate::collections::models::Collection;
use semantic_explorer_core::observability::DatabaseQueryTracker;
use semantic_explorer_core::owner_info::OwnerInfo;
use sqlx::types::chrono::{DateTime, Utc};

/// Helper struct for paginated queries that include total_count via COUNT(*) OVER()
#[derive(sqlx::FromRow)]
struct CollectionWithCount {
    pub collection_id: i32,
    pub title: String,
    pub details: Option<String>,
    pub owner_id: String,
    pub owner_display_name: String,
    pub tags: Vec<String>,
    pub is_public: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub file_count: i64,
    pub total_count: i64,
}

impl CollectionWithCount {
    fn into_parts(rows: Vec<Self>) -> (Vec<Collection>, i64) {
        let total_count = rows.first().map_or(0, |r| r.total_count);
        let collections = rows
            .into_iter()
            .map(|r| Collection {
                collection_id: r.collection_id,
                title: r.title,
                details: r.details,
                owner_id: r.owner_id,
                owner_display_name: r.owner_display_name,
                tags: r.tags,
                is_public: r.is_public,
                created_at: r.created_at,
                updated_at: r.updated_at,
                file_count: r.file_count,
            })
            .collect();
        (collections, total_count)
    }
}

const GET_COLLECTION_QUERY: &str = r#"
    SELECT c.collection_id, c.title, c.details, c.owner_id, c.owner_display_name, c.tags, c.is_public, c.created_at, c.updated_at, c.file_count
    FROM collections c
    WHERE c.collection_id = $1 AND c.owner_id = $2
"#;

const GET_COLLECTIONS_PAGINATED_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at, file_count,
        COUNT(*) OVER() AS total_count
    FROM collections
    WHERE owner_id = $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const SEARCH_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at, file_count,
        COUNT(*) OVER() AS total_count
    FROM collections
    WHERE owner_id = $1 AND (title ILIKE $2 OR details ILIKE $2 OR $3 = ANY(tags))
    ORDER BY created_at DESC
    LIMIT $4 OFFSET $5
"#;

const CREATE_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner_id, owner_display_name, tags, is_public)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING collection_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at, file_count
"#;

const DELETE_COLLECTION_QUERY: &str = r#"
    DELETE FROM collections WHERE collection_id = $1 AND owner_id = $2
"#;

const UPDATE_COLLECTION_QUERY: &str = r#"
    UPDATE collections
    SET title = $1, details = $2, tags = $3, is_public = $4, updated_at = NOW()
    WHERE collection_id = $5 AND owner_id = $6
    RETURNING collection_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at, file_count
"#;

const GET_PUBLIC_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at, file_count
    FROM collections
    WHERE is_public = TRUE
    ORDER BY created_at DESC
    LIMIT $1 OFFSET $2
"#;

const GET_RECENT_PUBLIC_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at, file_count
    FROM collections
    WHERE is_public = TRUE
    ORDER BY updated_at DESC
    LIMIT $1
"#;

const GRAB_PUBLIC_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner_id, owner_display_name, tags, is_public)
    SELECT title || '-grabbed', details, $1, $2, tags, FALSE
    FROM collections
    WHERE collection_id = $3 AND is_public = TRUE
    RETURNING collection_id, title, details, owner_id, owner_display_name, tags, is_public, created_at, updated_at, file_count
"#;
const INCREMENT_COLLECTION_FILE_COUNT_QUERY: &str = r#"
    UPDATE collections
    SET file_count = file_count + $2
    WHERE collection_id = $1
"#;

const DECREMENT_COLLECTION_FILE_COUNT_QUERY: &str = r#"
    UPDATE collections
    SET file_count = GREATEST(0, file_count - $2)
    WHERE collection_id = $1
"#;

#[tracing::instrument(name = "database.get_collection", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, collection_id = %collection_id))]
pub(crate) async fn get_collection(
    pool: &Pool<Postgres>,
    owner_id: &str,
    collection_id: i32,
) -> Result<Collection> {
    let tracker = DatabaseQueryTracker::new("SELECT", "collections");
    let result = sqlx::query_as::<_, Collection>(GET_COLLECTION_QUERY)
        .bind(collection_id)
        .bind(owner_id)
        .fetch_one(pool)
        .await;

    tracker.finish(result.is_ok());

    Ok(result?)
}

#[tracing::instrument(name = "database.get_collections_paginated", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, limit = %limit, offset = %offset))]
pub(crate) async fn get_collections_paginated(
    pool: &Pool<Postgres>,
    owner_id: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let tracker = DatabaseQueryTracker::new("SELECT", "collections");

    let result = sqlx::query_as::<_, CollectionWithCount>(GET_COLLECTIONS_PAGINATED_QUERY)
        .bind(owner_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    tracker.finish(result.is_ok());

    Ok(CollectionWithCount::into_parts(result?))
}

#[tracing::instrument(name = "database.search_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, query = %search_query, limit = %limit, offset = %offset))]
pub(crate) async fn search_collections(
    pool: &Pool<Postgres>,
    owner_id: &str,
    search_query: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let tracker = DatabaseQueryTracker::new("SELECT", "collections");
    let search_pattern = format!("%{}%", search_query);

    let result = sqlx::query_as::<_, CollectionWithCount>(SEARCH_COLLECTIONS_QUERY)
        .bind(owner_id)
        .bind(&search_pattern)
        .bind(search_query)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    tracker.finish(result.is_ok());

    Ok(CollectionWithCount::into_parts(result?))
}

#[tracing::instrument(name = "database.create_collection", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", title = %title, owner_id = %owner.owner_id))]
pub(crate) async fn create_collection(
    pool: &Pool<Postgres>,
    title: &str,
    details: Option<&str>,
    owner: &OwnerInfo,
    tags: &[String],
    is_public: bool,
) -> Result<Collection> {
    let tracker = DatabaseQueryTracker::new("INSERT", "collections");
    let result = sqlx::query_as::<_, Collection>(CREATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(&owner.owner_id)
        .bind(&owner.owner_display_name)
        .bind(tags)
        .bind(is_public)
        .fetch_one(pool)
        .await;

    tracker.finish(result.is_ok());

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_collection", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", collection_id = %collection_id, owner_id = %owner_id))]
pub(crate) async fn delete_collection(
    pool: &Pool<Postgres>,
    collection_id: i32,
    owner_id: &str,
) -> Result<()> {
    let tracker = DatabaseQueryTracker::new("DELETE", "collections");
    let result = sqlx::query(DELETE_COLLECTION_QUERY)
        .bind(collection_id)
        .bind(owner_id)
        .execute(pool)
        .await;

    tracker.finish(result.is_ok());

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.get_public_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_public_collections(
    pool: &Pool<Postgres>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Collection>> {
    let tracker = DatabaseQueryTracker::new("SELECT", "collections");
    let result = sqlx::query_as::<_, Collection>(GET_PUBLIC_COLLECTIONS_QUERY)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    tracker.finish(result.is_ok());

    Ok(result?)
}

#[tracing::instrument(name = "database.get_recent_public_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", limit = %limit))]
pub(crate) async fn get_recent_public_collections(
    pool: &Pool<Postgres>,
    limit: i32,
) -> Result<Vec<Collection>> {
    let tracker = DatabaseQueryTracker::new("SELECT", "collections");
    let result = sqlx::query_as::<_, Collection>(GET_RECENT_PUBLIC_COLLECTIONS_QUERY)
        .bind(limit)
        .fetch_all(pool)
        .await;

    tracker.finish(result.is_ok());

    Ok(result?)
}

#[tracing::instrument(name = "database.grab_public_collection", skip(pool, s3_client), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %owner_id, collection_id = %collection_id))]
pub(crate) async fn grab_public_collection(
    pool: &Pool<Postgres>,
    s3_client: &Client,
    s3_bucket_name: &str,
    owner_id: &str,
    owner_display_name: &str,
    collection_id: i32,
) -> Result<Collection> {
    let tracker = DatabaseQueryTracker::new("INSERT", "collections");

    // Insert and update in a single efficient query using CTE
    let result = sqlx::query_as::<_, Collection>(GRAB_PUBLIC_COLLECTION_QUERY)
        .bind(owner_id)
        .bind(owner_display_name)
        .bind(collection_id)
        .fetch_one(pool)
        .await;

    tracker.finish(result.is_ok());

    let new_collection = result?;

    match crate::storage::s3::copy_collection_files(
        s3_client,
        s3_bucket_name,
        collection_id,
        new_collection.collection_id,
    )
    .await
    {
        Ok(copied_count) => {
            tracing::info!(
                source_collection_id = %collection_id,
                destination_collection_id = %new_collection.collection_id,
                copied_count = copied_count,
                "Successfully copied S3 files for grabbed collection"
            );

            // Update file count for the new collection
            if let Err(e) = increment_collection_file_count(
                pool,
                new_collection.collection_id,
                copied_count as i64,
            )
            .await
            {
                tracing::error!(
                    collection_id = %new_collection.collection_id,
                    error = %e,
                    "Failed to update file count for grabbed collection"
                );
            }
        }
        Err(e) => {
            tracing::error!(
                source_collection_id = %collection_id,
                destination_collection_id = %new_collection.collection_id,
                error = %e,
                "Failed to copy S3 files for grabbed collection"
            );
            return Err(e);
        }
    }

    Ok(new_collection)
}

#[tracing::instrument(name = "database.update_collection", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", collection_id = %collection_id, owner_id = %owner_id))]
pub(crate) async fn update_collection(
    pool: &Pool<Postgres>,
    collection_id: i32,
    owner_id: &str,
    title: &str,
    details: Option<&str>,
    tags: &[String],
    is_public: bool,
) -> Result<Collection> {
    let tracker = DatabaseQueryTracker::new("UPDATE", "collections");
    let result = sqlx::query_as::<_, Collection>(UPDATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(tags)
        .bind(is_public)
        .bind(collection_id)
        .bind(owner_id)
        .fetch_one(pool)
        .await;

    tracker.finish(result.is_ok());

    Ok(result?)
}

#[tracing::instrument(name = "database.increment_collection_file_count", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT_UPDATE", collection_id = %collection_id, increment_by = %increment_by))]
pub(crate) async fn increment_collection_file_count(
    pool: &Pool<Postgres>,
    collection_id: i32,
    increment_by: i64,
) -> Result<()> {
    let tracker = DatabaseQueryTracker::new("UPDATE", "collections");
    let result = sqlx::query(INCREMENT_COLLECTION_FILE_COUNT_QUERY)
        .bind(collection_id)
        .bind(increment_by)
        .execute(pool)
        .await;

    tracker.finish(result.is_ok());

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.decrement_collection_file_count", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", collection_id = %collection_id, decrement_by = %decrement_by))]
pub(crate) async fn decrement_collection_file_count(
    pool: &Pool<Postgres>,
    collection_id: i32,
    decrement_by: i64,
) -> Result<()> {
    let tracker = DatabaseQueryTracker::new("UPDATE", "collections");
    let result = sqlx::query(DECREMENT_COLLECTION_FILE_COUNT_QUERY)
        .bind(collection_id)
        .bind(decrement_by)
        .execute(pool)
        .await;

    tracker.finish(result.is_ok());

    result?;
    Ok(())
}
