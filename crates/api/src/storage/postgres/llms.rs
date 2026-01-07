use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::llms::models::{CreateLLM, LargeLanguageModel, UpdateLargeLanguageModel};
use semantic_explorer_core::observability::record_database_query;

const GET_LLM_QUERY: &str = r#"
    SELECT llm_id, name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at
    FROM llms
    WHERE owner = $1 AND llm_id = $2
"#;

const GET_LLMS_QUERY: &str = r#"
    SELECT llm_id, name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at
    FROM llms
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

const GET_PUBLIC_LLMS_QUERY: &str = r#"
    SELECT llm_id, name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at
    FROM llms
    WHERE is_public = TRUE
    ORDER BY created_at DESC
"#;

const CREATE_LLM_QUERY: &str = r#"
    INSERT INTO llms (name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
    RETURNING llm_id, name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at
"#;

const DELETE_LLM_QUERY: &str = r#"
    DELETE FROM llms WHERE owner = $1 AND llm_id = $2
"#;

const UPDATE_LLM_QUERY: &str = r#"
    UPDATE llms
    SET name = COALESCE($3, name),
        base_url = COALESCE($4, base_url),
        api_key = COALESCE($5, api_key),
        config = COALESCE($6, config),
        is_public = COALESCE($7, is_public),
        updated_at = NOW()
    WHERE owner = $1 AND llm_id = $2
    RETURNING llm_id, name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at
"#;

const GRAB_PUBLIC_LLM_QUERY: &str = r#"
    INSERT INTO llms (name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at)
    SELECT name, $1, provider, base_url, api_key, config, FALSE, NOW(), NOW()
    FROM llms
    WHERE llm_id = $2 AND is_public = TRUE
    RETURNING llm_id, name, owner, provider, base_url, api_key, config, is_public, created_at, updated_at
"#;

#[tracing::instrument(name = "database.get_llm", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner, llm_id = %llm_id))]
pub(crate) async fn get_llm(
    pool: &Pool<Postgres>,
    owner: &str,
    llm_id: i32,
) -> Result<LargeLanguageModel> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GET_LLM_QUERY)
        .bind(owner)
        .bind(llm_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_llms", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_llms(
    pool: &Pool<Postgres>,
    owner: &str,
) -> Result<Vec<LargeLanguageModel>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GET_LLMS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_public_llms", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_public_llms(pool: &Pool<Postgres>) -> Result<Vec<LargeLanguageModel>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GET_PUBLIC_LLMS_QUERY)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.create_llm", skip(pool, create_llm), fields(database.system = "postgresql", database.operation = "INSERT", owner = %owner))]
pub(crate) async fn create_llm(
    pool: &Pool<Postgres>,
    owner: &str,
    create_llm: &CreateLLM,
) -> Result<LargeLanguageModel> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(CREATE_LLM_QUERY)
        .bind(&create_llm.name)
        .bind(owner)
        .bind(&create_llm.provider)
        .bind(&create_llm.base_url)
        .bind(&create_llm.api_key)
        .bind(&create_llm.config)
        .bind(create_llm.is_public)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "llms", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_llm", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", owner = %owner, llm_id = %llm_id))]
pub(crate) async fn delete_llm(pool: &Pool<Postgres>, owner: &str, llm_id: i32) -> Result<()> {
    let start = Instant::now();
    let result = sqlx::query(DELETE_LLM_QUERY)
        .bind(owner)
        .bind(llm_id)
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "llms", duration, success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.update_llm", skip(pool, update_llm), fields(database.system = "postgresql", database.operation = "UPDATE", owner = %owner, llm_id = %llm_id))]
pub(crate) async fn update_llm(
    pool: &Pool<Postgres>,
    owner: &str,
    llm_id: i32,
    update_llm: &UpdateLargeLanguageModel,
) -> Result<LargeLanguageModel> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(UPDATE_LLM_QUERY)
        .bind(owner)
        .bind(llm_id)
        .bind(&update_llm.name)
        .bind(&update_llm.base_url)
        .bind(&update_llm.api_key)
        .bind(&update_llm.config)
        .bind(update_llm.is_public)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "llms", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.grab_public_llm", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", owner = %owner, llm_id = %llm_id))]
pub(crate) async fn grab_public_llm(
    pool: &Pool<Postgres>,
    owner: &str,
    llm_id: i32,
) -> Result<LargeLanguageModel> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GRAB_PUBLIC_LLM_QUERY)
        .bind(owner)
        .bind(llm_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "llms", duration, success);

    Ok(result?)
}
