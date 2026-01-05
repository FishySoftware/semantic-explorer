use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::collections::models::Collection;
use semantic_explorer_core::observability::record_database_query;

const GET_COLLECTION_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, created_at, updated_at FROM collections
    WHERE owner = $1 AND collection_id = $2
"#;

const GET_COLLECTIONS_QUERY: &str = r#"
    SELECT collection_id, title, details, owner, bucket, tags, created_at, updated_at FROM collections WHERE owner = $1
"#;

const CREATE_COLLECTION_QUERY: &str = r#"
    INSERT INTO collections (title, details, owner, bucket, tags)
    VALUES ($1, $2, $3, $4, $5)
    RETURNING collection_id, title, details, owner, bucket, tags, created_at, updated_at
"#;

const DELETE_COLLECTION_QUERY: &str = r#"
    DELETE FROM collections WHERE owner = $1 AND collection_id = $2
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

#[tracing::instrument(name = "database.create_collection", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", title = %title, owner = %owner))]
pub(crate) async fn create_collection(
    pool: &Pool<Postgres>,
    title: &str,
    details: Option<&str>,
    owner: &str,
    bucket: &str,
    tags: &[String],
) -> Result<Collection> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Collection>(CREATE_COLLECTION_QUERY)
        .bind(title)
        .bind(details)
        .bind(owner)
        .bind(bucket)
        .bind(tags)
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
