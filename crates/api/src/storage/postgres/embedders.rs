use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::embedders::models::{CreateEmbedder, Embedder, UpdateEmbedder};
use semantic_explorer_core::observability::record_database_query;

const GET_EMBEDDER_QUERY: &str = r#"
    SELECT embedder_id, name, owner, provider, base_url, api_key, config, batch_size, max_batch_size, dimensions, collection_name, created_at, updated_at 
    FROM embedders
    WHERE owner = $1 AND embedder_id = $2
"#;

const GET_EMBEDDERS_QUERY: &str = r#"
    SELECT embedder_id, name, owner, provider, base_url, api_key, config, batch_size, max_batch_size, dimensions, collection_name, created_at, updated_at
    FROM embedders
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

// const GET_EMBEDDERS_BY_IDS_QUERY: &str = r#"
//     SELECT embedder_id, name, owner, provider, base_url, api_key, config, batch_size, max_batch_size, dimensions, collection_name, created_at, updated_at
//     FROM embedders
//     WHERE owner = $1 AND embedder_id = ANY($2)
// "#;

const CREATE_EMBEDDER_QUERY: &str = r#"
    INSERT INTO embedders (name, owner, provider, base_url, api_key, config, batch_size, max_batch_size, dimensions, collection_name, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
    RETURNING embedder_id, name, owner, provider, base_url, api_key, config, batch_size, max_batch_size, dimensions, collection_name, created_at, updated_at
"#;

const DELETE_EMBEDDER_QUERY: &str = r#"
    DELETE FROM embedders WHERE owner = $1 AND embedder_id = $2
"#;

const UPDATE_EMBEDDER_QUERY: &str = r#"
    UPDATE embedders 
    SET name = COALESCE($3, name),
        base_url = COALESCE($4, base_url),
        api_key = COALESCE($5, api_key),
        config = COALESCE($6, config),
        batch_size = COALESCE($7, batch_size),
        max_batch_size = COALESCE($8, max_batch_size),
        dimensions = COALESCE($9, dimensions),
        collection_name = COALESCE($10, collection_name),
        updated_at = NOW()
    WHERE owner = $1 AND embedder_id = $2
    RETURNING embedder_id, name, owner, provider, base_url, api_key, config, batch_size, max_batch_size, dimensions, collection_name, created_at, updated_at
"#;

#[tracing::instrument(name = "database.get_embedder", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner, embedder_id = %embedder_id))]
pub(crate) async fn get_embedder(
    pool: &Pool<Postgres>,
    owner: &str,
    embedder_id: i32,
) -> Result<Embedder> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(GET_EMBEDDER_QUERY)
        .bind(owner)
        .bind(embedder_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_embedders", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_embedders(pool: &Pool<Postgres>, owner: &str) -> Result<Vec<Embedder>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(GET_EMBEDDERS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    Ok(result?)
}

// #[tracing::instrument(name = "database.get_embedders_by_ids", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner, count = embedder_ids.len()))]
// pub(crate) async fn get_embedders_by_ids(
//     pool: &Pool<Postgres>,
//     owner: &str,
//     embedder_ids: &[i32],
// ) -> Result<Vec<Embedder>> {
//     let start = Instant::now();
//     let result = sqlx::query_as::<_, Embedder>(GET_EMBEDDERS_BY_IDS_QUERY)
//         .bind(owner)
//         .bind(embedder_ids)
//         .fetch_all(pool)
//         .await;

//     let duration = start.elapsed().as_secs_f64();
//     let success = result.is_ok();
//     record_database_query("SELECT", "embedders", duration, success);

//     Ok(result?)
// }

#[tracing::instrument(name = "database.create_embedder", skip(pool, create_embedder), fields(database.system = "postgresql", database.operation = "INSERT", owner = %owner))]
pub(crate) async fn create_embedder(
    pool: &Pool<Postgres>,
    owner: &str,
    create_embedder: &CreateEmbedder,
) -> Result<Embedder> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(CREATE_EMBEDDER_QUERY)
        .bind(&create_embedder.name)
        .bind(owner)
        .bind(&create_embedder.provider)
        .bind(&create_embedder.base_url)
        .bind(&create_embedder.api_key)
        .bind(&create_embedder.config)
        .bind(create_embedder.batch_size)
        .bind(create_embedder.max_batch_size)
        .bind(create_embedder.dimensions)
        .bind(&create_embedder.collection_name)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "embedders", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_embedder", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", owner = %owner, embedder_id = %embedder_id))]
pub(crate) async fn delete_embedder(
    pool: &Pool<Postgres>,
    owner: &str,
    embedder_id: i32,
) -> Result<()> {
    let start = Instant::now();
    let result = sqlx::query(DELETE_EMBEDDER_QUERY)
        .bind(owner)
        .bind(embedder_id)
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "embedders", duration, success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.update_embedder", skip(pool, update_embedder), fields(database.system = "postgresql", database.operation = "UPDATE", owner = %owner, embedder_id = %embedder_id))]
pub(crate) async fn update_embedder(
    pool: &Pool<Postgres>,
    owner: &str,
    embedder_id: i32,
    update_embedder: &UpdateEmbedder,
) -> Result<Embedder> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(UPDATE_EMBEDDER_QUERY)
        .bind(owner)
        .bind(embedder_id)
        .bind(&update_embedder.name)
        .bind(&update_embedder.base_url)
        .bind(&update_embedder.api_key)
        .bind(&update_embedder.config)
        .bind(update_embedder.batch_size)
        .bind(update_embedder.max_batch_size)
        .bind(update_embedder.dimensions)
        .bind(&update_embedder.collection_name)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "embedders", duration, success);

    Ok(result?)
}
