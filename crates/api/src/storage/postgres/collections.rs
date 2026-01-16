use anyhow::Result;
use aws_sdk_s3::Client;
use sqlx::{Pool, Postgres};
use std::time::Instant;
use uuid::Uuid;

use crate::collections::models::Collection;
use semantic_explorer_core::observability::record_database_query;

// SQL queries - RLS policies handle owner filtering automatically
const GET_COLLECTION_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at 
    FROM collections
    WHERE collection_id = $1
"#;

const GET_COLLECTIONS_PAGINATED_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
    FROM collections
    ORDER BY created_at DESC
    LIMIT $1 OFFSET $2
"#;

const COUNT_COLLECTIONS_QUERY: &str = r#"
    SELECT COUNT(*) as count FROM collections
"#;

const SEARCH_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE owner_id = $1 AND (title ILIKE $2 OR details ILIKE $2 OR $3 = ANY(tags))
    ORDER BY created_at DESC
    LIMIT $4 OFFSET $5
"#;

const COUNT_SEARCH_COLLECTIONS_QUERY: &str = r#"
    SELECT COUNT(*) as count
    FROM collections
    WHERE owner_id = $1 AND (title ILIKE $2 OR details ILIKE $2 OR $3 = ANY(tags))
"#;

const CREATE_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner_id, owner_display_name, bucket, tags, is_public)
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    RETURNING collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
"#;

const DELETE_COLLECTION_QUERY: &str = r#"
    DELETE FROM collections WHERE collection_id = $1
"#;

const UPDATE_COLLECTION_QUERY: &str = r#"
    UPDATE collections
    SET title = $1, details = $2, tags = $3, is_public = $4, updated_at = NOW()
    WHERE collection_id = $5
    RETURNING collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
"#;

// Public collections don't require RLS context
const GET_PUBLIC_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE is_public = TRUE
    ORDER BY created_at DESC
    LIMIT 1000
"#;

const GET_RECENT_PUBLIC_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE is_public = TRUE
    ORDER BY updated_at DESC
    LIMIT $1
"#;

const GRAB_PUBLIC_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner_id, owner_display_name, bucket, tags, is_public)
    SELECT title || ' - grabbed', details, $1, $2, $3, tags, FALSE
    FROM collections
    WHERE collection_id = $4 AND is_public = TRUE
    RETURNING collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
"#;

const GET_PUBLIC_COLLECTION_QUERY: &str = r#"
    SELECT collection_id, title, details, owner_id, owner_display_name, bucket, tags, is_public, created_at, updated_at
    FROM collections
    WHERE collection_id = $1 AND is_public = TRUE
"#;

const UPSERT_COLLECTION_FILE_COUNT_QUERY: &str = r#"
    INSERT INTO collection_file_counts (collection_id, file_count, cached_at)
    VALUES ($1, $2, NOW())
    ON CONFLICT (collection_id)
    DO UPDATE SET file_count = $2, cached_at = NOW()
"#;

const GET_COLLECTION_FILE_COUNT_QUERY: &str = r#"
    SELECT file_count FROM collection_file_counts WHERE collection_id = $1
"#;

const GET_COLLECTIONS_FILE_COUNTS_BATCH_QUERY: &str = r#"
    SELECT collection_id, file_count FROM collection_file_counts WHERE collection_id = ANY($1)
"#;

#[tracing::instrument(name = "database.get_collection", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, collection_id = %collection_id))]
pub(crate) async fn get_collection(
    pool: &Pool<Postgres>,
    owner_id: &str,
    collection_id: i32,
) -> Result<Collection> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

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

#[tracing::instrument(name = "database.get_collections_paginated", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, limit = %limit, offset = %offset))]
pub(crate) async fn get_collections_paginated(
    pool: &Pool<Postgres>,
    owner_id: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

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

#[tracing::instrument(name = "database.search_collections", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, query = %search_query, limit = %limit, offset = %offset))]
pub(crate) async fn search_collections(
    pool: &Pool<Postgres>,
    owner_id: &str,
    search_query: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Collection>, i64)> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let start = Instant::now();
    let search_pattern = format!("%{}%", search_query);

    // Get search results
    let collections_result = sqlx::query_as::<_, Collection>(SEARCH_COLLECTIONS_QUERY)
        .bind(owner_id)
        .bind(&search_pattern)
        .bind(search_query)
        .bind(limit)
        .bind(offset)
        .fetch_all(&mut *tx)
        .await;

    // Get total count of search results
    let count_result: Result<(i64,), sqlx::Error> = sqlx::query_as(COUNT_SEARCH_COLLECTIONS_QUERY)
        .bind(owner_id)
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

#[tracing::instrument(name = "database.create_collection", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", title = %title, owner_id = %owner_id))]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_collection(
    pool: &Pool<Postgres>,
    title: &str,
    details: Option<&str>,
    owner_id: &str,
    owner_display_name: &str,
    bucket: &str,
    tags: &[String],
    is_public: bool,
) -> Result<Collection> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(CREATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(owner_id)
        .bind(owner_display_name)
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

#[tracing::instrument(name = "database.delete_collection", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", collection_id = %collection_id, owner_id = %owner_id))]
pub(crate) async fn delete_collection(
    pool: &Pool<Postgres>,
    collection_id: i32,
    owner_id: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

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

#[tracing::instrument(name = "database.update_collection_bucket", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", collection_id = %collection_id, owner_id = %owner_id))]
pub(crate) async fn update_collection_bucket(
    pool: &Pool<Postgres>,
    collection_id: i32,
    owner_id: &str,
    bucket: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let start = Instant::now();
    let result = sqlx::query("UPDATE collections SET bucket = $1 WHERE collection_id = $2")
        .bind(bucket)
        .bind(collection_id)
        .execute(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "collections", duration, success);

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

#[tracing::instrument(name = "database.grab_public_collection", skip(pool, s3_client), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %owner_id, collection_id = %collection_id))]
pub(crate) async fn grab_public_collection(
    pool: &Pool<Postgres>,
    s3_client: &Client,
    owner_id: &str,
    owner_display_name: &str,
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
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let result = sqlx::query_as::<_, Collection>(GRAB_PUBLIC_COLLECTION_QUERY)
        .bind(owner_id)
        .bind(owner_display_name)
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
    match crate::storage::s3::copy_bucket_files(s3_client, &source_collection.bucket, &new_bucket)
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
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

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

#[tracing::instrument(name = "database.upsert_collection_file_count", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT_UPDATE", collection_id = %collection_id, file_count = %file_count))]
pub(crate) async fn upsert_collection_file_count(
    pool: &Pool<Postgres>,
    collection_id: i32,
    file_count: i64,
) -> Result<()> {
    let start = Instant::now();
    let result = sqlx::query(UPSERT_COLLECTION_FILE_COUNT_QUERY)
        .bind(collection_id)
        .bind(file_count)
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT_UPDATE", "collection_file_counts", duration, success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.get_collection_file_count", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", collection_id = %collection_id))]
pub(crate) async fn get_collection_file_count(
    pool: &Pool<Postgres>,
    collection_id: i32,
) -> Result<Option<i64>> {
    let start = Instant::now();
    let result: Option<(i64,)> = sqlx::query_as(GET_COLLECTION_FILE_COUNT_QUERY)
        .bind(collection_id)
        .fetch_optional(pool)
        .await?;

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "collection_file_counts", duration, true);

    Ok(result.map(|(count,)| count))
}

#[tracing::instrument(name = "database.get_collections_file_counts_batch", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_collections_file_counts_batch(
    pool: &Pool<Postgres>,
    collection_ids: Vec<i32>,
) -> Result<std::collections::HashMap<i32, i64>> {
    if collection_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let start = Instant::now();
    let results: Vec<(i32, i64)> = sqlx::query_as(GET_COLLECTIONS_FILE_COUNTS_BATCH_QUERY)
        .bind(&collection_ids)
        .fetch_all(pool)
        .await?;

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "collection_file_counts", duration, true);

    Ok(results.into_iter().collect())
}

#[tracing::instrument(name = "storage.count_s3_files_for_collection", skip(s3_client), fields(bucket = %bucket, owner_id = %owner_id, collection_id = %collection_id))]
pub(crate) async fn count_s3_files_for_collection(
    s3_client: &Client,
    bucket: &str,
    owner_id: &str,
    collection_id: i32,
) -> Result<i64> {
    let prefix = format!("{}/{}/", owner_id, collection_id);
    let mut count: i64 = 0;
    let mut continuation_token: Option<String> = None;

    loop {
        let mut request = s3_client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(&prefix)
            .max_keys(1000);

        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }

        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to list S3 objects: {}", e))?;

        count += response.contents().len() as i64;

        if response.is_truncated().unwrap_or(false) {
            continuation_token = response.next_continuation_token().map(|s| s.to_string());
        } else {
            break;
        }
    }

    Ok(count)
}
