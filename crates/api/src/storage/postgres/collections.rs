use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::collections::models::Collection;
use semantic_explorer_core::observability::record_database_query;

const GET_COLLECTION_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at FROM collections
    WHERE owner = $1 AND collection_id = $2
"#;

const GET_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at FROM collections WHERE owner = $1
"#;

const GET_COLLECTIONS_PAGINATED_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE owner = $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const COUNT_COLLECTIONS_QUERY: &str = r#"
    SELECT COUNT(*) as count FROM collections WHERE owner = $1
"#;

const SEARCH_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE owner = $1 AND (title ILIKE $2 OR details ILIKE $2 OR $3 = ANY(tags))
    ORDER BY created_at DESC
    LIMIT $4 OFFSET $5
"#;

const COUNT_SEARCH_COLLECTIONS_QUERY: &str = r#"
    SELECT COUNT(*) as count
    FROM collections
    WHERE owner = $1 AND (title ILIKE $2 OR details ILIKE $2 OR $3 = ANY(tags))
"#;

const CREATE_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner, bucket, tags, is_public)
    VALUES ($1, $2, $3, $4, $5, $6)
    RETURNING collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
"#;

const DELETE_COLLECTION_QUERY: &str = r#"
    DELETE FROM collections WHERE owner = $1 AND collection_id = $2
"#;

const GET_PUBLIC_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE is_public = TRUE
    ORDER BY created_at DESC
"#;

const GRAB_PUBLIC_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner, bucket, tags, is_public)
    SELECT title, details, $1, $2, tags, FALSE
    FROM collections
    WHERE collection_id = $3 AND is_public = TRUE
    RETURNING collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
"#;

const UPDATE_COLLECTION_QUERY: &str = r#"
    UPDATE collections
    SET title = $1, details = $2, tags = $3, is_public = $4, updated_at = NOW()
    WHERE collection_id = $5 AND owner = $6
    RETURNING collection_id, title, details, owner, bucket, tags, is_public, created_at, updated_at
"#;

#[tracing::instrument(name = "database.get_collection", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner, collection_id = %collection_id))]
pub(crate) async fn get_collection(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_id: i32,
) -> Result<Collection> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(GET_COLLECTION_QUERY)
        .bind(owner)
        .bind(collection_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_collections(pool: &Pool<Postgres>, owner: &str) -> Result<Vec<Collection>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(GET_COLLECTIONS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_collections_paginated", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner, limit = %limit, offset = %offset))]
pub(crate) async fn get_collections_paginated(
    pool: &Pool<Postgres>,
    owner: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let start = Instant::now();

    // Get paginated collections
    let collections_result = sqlx::query_as::<_, Collection>(GET_COLLECTIONS_PAGINATED_QUERY)
        .bind(owner)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    // Get total count
    let count_result: Result<(i64,), sqlx::Error> = sqlx::query_as(COUNT_COLLECTIONS_QUERY)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = collections_result.is_ok() && count_result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    let collections = collections_result?;
    let (total_count,) = count_result?;

    Ok((collections, total_count))
}

#[tracing::instrument(name = "database.search_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner, query = %search_query, limit = %limit, offset = %offset))]
pub(crate) async fn search_collections(
    pool: &Pool<Postgres>,
    owner: &str,
    search_query: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let start = Instant::now();
    let search_pattern = format!("%{}%", search_query);

    // Get search results
    let collections_result = sqlx::query_as::<_, Collection>(SEARCH_COLLECTIONS_QUERY)
        .bind(owner)
        .bind(&search_pattern)
        .bind(search_query)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    // Get total count of search results
    let count_result: Result<(i64,), sqlx::Error> = sqlx::query_as(COUNT_SEARCH_COLLECTIONS_QUERY)
        .bind(owner)
        .bind(&search_pattern)
        .bind(search_query)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = collections_result.is_ok() && count_result.is_ok();
    record_database_query("SELECT", "collections", duration, success);

    let collections = collections_result?;
    let (total_count,) = count_result?;

    Ok((collections, total_count))
}

#[tracing::instrument(name = "database.create_collection", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", title = %title, owner = %owner))]
pub(crate) async fn create_collection(
    pool: &Pool<Postgres>,
    title: &str,
    details: Option<&str>,
    owner: &str,
    bucket: &str,
    tags: &[String],
    is_public: bool,
) -> Result<Collection> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(CREATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(owner)
        .bind(bucket)
        .bind(tags)
        .bind(is_public)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "collections", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_collection", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", collection_id = %collection_id, owner = %owner))]
pub(crate) async fn delete_collection(
    pool: &Pool<Postgres>,
    collection_id: i32,
    owner: &str,
) -> Result<()> {
    let start = Instant::now();
    let result = sqlx::query(DELETE_COLLECTION_QUERY)
        .bind(owner)
        .bind(collection_id)
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "collections", duration, success);

    result?;
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

#[tracing::instrument(name = "database.grab_public_collection", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", owner = %owner, collection_id = %collection_id))]
pub(crate) async fn grab_public_collection(
    pool: &Pool<Postgres>,
    owner: &str,
    collection_id: i32,
) -> Result<Collection> {
    let start = Instant::now();

    // Generate a unique bucket name for the new collection
    let bucket = format!("{}_{}", owner, uuid::Uuid::new_v4());

    let result = sqlx::query_as::<_, Collection>(GRAB_PUBLIC_COLLECTION_QUERY)
        .bind(owner)
        .bind(&bucket)
        .bind(collection_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "collections", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.update_collection", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", collection_id = %collection_id, owner = %owner))]
pub(crate) async fn update_collection(
    pool: &Pool<Postgres>,
    collection_id: i32,
    owner: &str,
    title: &str,
    details: Option<&str>,
    tags: &[String],
    is_public: bool,
) -> Result<Collection> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(UPDATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(tags)
        .bind(is_public)
        .bind(collection_id)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "collections", duration, success);

    Ok(result?)
}
