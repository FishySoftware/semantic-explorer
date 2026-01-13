use anyhow::Result;
use aws_sdk_s3::Client;
use sqlx::{Pool, Postgres};
use std::time::Instant;
use uuid::Uuid;

use crate::collections::models::Collection;
use semantic_explorer_core::observability::record_database_query;

// SQL queries - RLS policies handle owner filtering automatically
const GET_COLLECTION_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at 
    FROM collections
    WHERE collection_id = $1
"#;

const GET_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at 
    FROM collections
    ORDER BY created_at DESC
    LIMIT 1000
"#;

const GET_COLLECTIONS_PAGINATED_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    ORDER BY created_at DESC
    LIMIT $1 OFFSET $2
"#;

const COUNT_COLLECTIONS_QUERY: &str = r#"
    SELECT COUNT(*) as count FROM collections
"#;

const SEARCH_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE (title ILIKE $1 OR details ILIKE $1 OR $2 = ANY(tags))
    ORDER BY created_at DESC
    LIMIT $3 OFFSET $4
"#;

const COUNT_SEARCH_COLLECTIONS_QUERY: &str = r#"
    SELECT COUNT(*) as count
    FROM collections
    WHERE (title ILIKE $1 OR details ILIKE $1 OR $2 = ANY(tags))
"#;

const CREATE_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner, bucket, tags, is_public)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
"#;

const DELETE_COLLECTION_QUERY: &str = r#"
    DELETE FROM collections WHERE collection_id = $1
"#;

const UPDATE_COLLECTION_QUERY: &str = r#"
    UPDATE collections
    SET title = $1, details = $2, tags = $3, is_public = $4, updated_at = NOW()
    WHERE collection_id = $5
    RETURNING collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
"#;

// Public collections don't require RLS context
const GET_PUBLIC_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE is_public = TRUE
    ORDER BY created_at DESC
    LIMIT 1000
"#;

const GET_RECENT_PUBLIC_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE is_public = TRUE
    ORDER BY updated_at DESC
    LIMIT $1
"#;

const GRAB_PUBLIC_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner, bucket, tags, is_public)
    SELECT title || ' - grabbed', details, $1, $2, tags, FALSE
    FROM collections
    WHERE collection_id = $3 AND is_public = TRUE
    RETURNING collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
"#;

const GET_PUBLIC_COLLECTION_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE collection_id = $1 AND is_public = TRUE
"#;

#[tracing::instrument(name = "database.get_collection", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", username = %username, collection_id = %collection_id))]
pub(crate) async fn get_collection(
    pool: &Pool<Postgres>,
    username: &str,
    collection_id: i32,
) -> Result<Collection> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(GET_COLLECTION_QUERY)
        .bind(collection_id)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    let collection = result?;
    tx.commit().await?;
    Ok(collection)
}

#[tracing::instrument(name = "database.get_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", username = %username))]
pub(crate) async fn get_collections(
    pool: &Pool<Postgres>,
    username: &str,
) -> Result<Vec<Collection>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(GET_COLLECTIONS_QUERY)
        .fetch_all(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    let collections = result?;
    tx.commit().await?;
    Ok(collections)
}

#[tracing::instrument(name = "database.get_collections_paginated", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", username = %username, limit = %limit, offset = %offset))]
pub(crate) async fn get_collections_paginated(
    pool: &Pool<Postgres>,
    username: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let start = Instant::now();

    // Get paginated collections
    let collections_result = sqlx::query_as::<_, Collection>(GET_COLLECTIONS_PAGINATED_QUERY)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await;

    // Get total count
    let count_result: Result<(i64,), sqlx::Error> = sqlx::query_as(COUNT_COLLECTIONS_QUERY)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = collections_result.is_ok() && count_result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    let collections = collections_result?;
    let (total_count,) = count_result?;

    tx.commit().await?;
    Ok((collections, total_count))
}

#[tracing::instrument(name = "database.search_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", username = %username, query = %search_query, limit = %limit, offset = %offset))]
pub(crate) async fn search_collections(
    pool: &Pool<Postgres>,
    username: &str,
    search_query: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let start = Instant::now();
    let search_pattern = format!("%{}%", search_query);

    // Get search results
    let collections_result = sqlx::query_as::<_, Collection>(SEARCH_COLLECTIONS_QUERY)
        .bind(&search_pattern)
        .bind(search_query)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await;

    // Get total count of search results
    let count_result: Result<(i64,), sqlx::Error> = sqlx::query_as(COUNT_SEARCH_COLLECTIONS_QUERY)
        .bind(&search_pattern)
        .bind(search_query)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = collections_result.is_ok() && count_result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    let collections = collections_result?;
    let (total_count,) = count_result?;

    tx.commit().await?;
    Ok((collections, total_count))
}

#[tracing::instrument(name = "database.create_collection", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", title = %title, username = %username))]
pub(crate) async fn create_collection(
    pool: &Pool<Postgres>,
    title: &str,
    details: Option<&str>,
    username: &str,
    bucket: &str,
    tags: &[String],
    is_public: bool,
) -> Result<Collection> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(CREATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(username)
        .bind(bucket)
        .bind(tags)
        .bind(is_public)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "collections", duration, success);

    let collection = result?;
    tx.commit().await?;
    Ok(collection)
}

#[tracing::instrument(name = "database.delete_collection", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", collection_id = %collection_id, username = %username))]
pub(crate) async fn delete_collection(
    pool: &Pool<Postgres>,
    collection_id: i32,
    username: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let start = Instant::now();
    let result = sqlx::query(DELETE_COLLECTION_QUERY)
        .bind(collection_id)
        .execute(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "collections", duration, success);

    result?;
    tx.commit().await?;
    Ok(())
}

#[tracing::instrument(name = "database.get_public_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_public_collections(pool: &Pool<Postgres>) -> Result<Vec<Collection>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(GET_PUBLIC_COLLECTIONS_QUERY)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_recent_public_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", limit = %limit))]
pub(crate) async fn get_recent_public_collections(
    pool: &Pool<Postgres>,
    limit: i32,
) -> Result<Vec<Collection>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(GET_RECENT_PUBLIC_COLLECTIONS_QUERY)
        .bind(limit)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.grab_public_collection", skip(pool, s3_client), fields(database.system = "postgresql", database.operation = "INSERT", username = %username, collection_id = %collection_id))]
pub(crate) async fn grab_public_collection(
    pool: &Pool<Postgres>,
    s3_client: &Client,
    username: &str,
    collection_id: i32,
) -> Result<Collection> {
    let start = Instant::now();

    // First, get the source collection to find its bucket (public, no RLS needed)
    let source_collection = sqlx::query_as::<_, Collection>(GET_PUBLIC_COLLECTION_QUERY)
        .bind(collection_id)
        .fetch_one(pool)
        .await?;

    // Generate a unique bucket name for the new collection
    let new_bucket = Uuid::new_v4().to_string();

    // Insert the new collection with RLS context
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let result = sqlx::query_as::<_, Collection>(GRAB_PUBLIC_COLLECTION_QUERY)
        .bind(username)
        .bind(&new_bucket)
        .bind(collection_id)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "collections", duration, success);

    let new_collection = result?;
    tx.commit().await?;

    // Copy S3 files from source bucket to new bucket
    match crate::storage::rustfs::copy_bucket_files(
        s3_client,
        &source_collection.bucket,
        &new_bucket,
    )
    .await
    {
        Ok(copied_count) => {
            tracing::info!(
                source_bucket = %source_collection.bucket,
                destination_bucket = %new_bucket,
                copied_count = copied_count,
                "Successfully copied S3 files for grabbed collection"
            );
        }
        Err(e) => {
            tracing::error!(
                source_bucket = %source_collection.bucket,
                destination_bucket = %new_bucket,
                error = %e,
                "Failed to copy S3 files for grabbed collection"
            );
            return Err(e);
        }
    }

    Ok(new_collection)
}

#[tracing::instrument(name = "database.update_collection", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", collection_id = %collection_id, username = %username))]
pub(crate) async fn update_collection(
    pool: &Pool<Postgres>,
    collection_id: i32,
    username: &str,
    title: &str,
    details: Option<&str>,
    tags: &[String],
    is_public: bool,
) -> Result<Collection> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, username).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(UPDATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(tags)
        .bind(is_public)
        .bind(collection_id)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "collections", duration, success);

    let collection = result?;
    tx.commit().await?;
    Ok(collection)
}
