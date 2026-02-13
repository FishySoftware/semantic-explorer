use anyhow::Result;
use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::auth::AuthenticatedUser;
use crate::llms::models::{CreateLLM, LargeLanguageModel, UpdateLargeLanguageModel};
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::observability::record_database_query;
use sqlx::types::chrono::{DateTime, Utc};

pub(crate) struct PaginatedResult<T> {
    pub(crate) items: Vec<T>,
    pub(crate) total_count: i64,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

/// Helper struct for paginated queries that include total_count via COUNT(*) OVER()
#[derive(sqlx::FromRow)]
struct LlmWithCount {
    pub llm_id: i32,
    pub name: String,
    pub owner_id: String,
    pub owner_display_name: String,
    pub provider: String,
    pub base_url: String,
    #[sqlx(rename = "api_key_encrypted")]
    pub api_key: Option<String>,
    pub config: serde_json::Value,
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_count: i64,
}

impl LlmWithCount {
    fn into_parts(rows: Vec<Self>) -> (Vec<LargeLanguageModel>, i64) {
        let total_count = rows.first().map_or(0, |r| r.total_count);
        let llms = rows
            .into_iter()
            .map(|r| LargeLanguageModel {
                llm_id: r.llm_id,
                name: r.name,
                owner_id: r.owner_id,
                owner_display_name: r.owner_display_name,
                provider: r.provider,
                base_url: r.base_url,
                api_key: r.api_key,
                config: r.config,
                is_public: r.is_public,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();
        (llms, total_count)
    }
}

const GET_LLM_QUERY: &str = r#"
    SELECT llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at
    FROM llms
    WHERE llm_id = $1 AND owner_id = $2
"#;

const GET_LLMS_QUERY: &str = r#"
    SELECT llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at,
        COUNT(*) OVER() AS total_count
    FROM llms
    WHERE owner_id = $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const GET_LLMS_WITH_SEARCH_QUERY: &str = r#"
    SELECT llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at,
        COUNT(*) OVER() AS total_count
    FROM llms
    WHERE owner_id = $1 AND name ILIKE $2
    ORDER BY created_at DESC
    LIMIT $3 OFFSET $4
"#;

const GET_PUBLIC_LLMS_QUERY: &str = r#"
    SELECT llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at
    FROM llms
    WHERE is_public = TRUE
    ORDER BY created_at DESC
"#;

const GET_RECENT_PUBLIC_LLMS_QUERY: &str = r#"
    SELECT llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at
    FROM llms
    WHERE is_public = TRUE
    ORDER BY updated_at DESC
    LIMIT $1
"#;

const CREATE_LLM_QUERY: &str = r#"
    INSERT INTO llms (name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, model, config, is_public, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())
    RETURNING llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at
"#;

const DELETE_LLM_QUERY: &str = r#"
    DELETE FROM llms WHERE llm_id = $1 AND owner_id = $2
"#;

const UPDATE_LLM_QUERY: &str = r#"
    UPDATE llms
    SET name = COALESCE($2, name),
        base_url = COALESCE($3, base_url),
        api_key_encrypted = COALESCE($4, api_key_encrypted),
        config = COALESCE($5, config),
        is_public = COALESCE($6, is_public),
        updated_at = NOW()
    WHERE llm_id = $1 AND owner_id = $7
    RETURNING llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at
"#;

const GRAB_PUBLIC_LLM_QUERY: &str = r#"
    INSERT INTO llms (name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, model, config, is_public, created_at, updated_at)
    SELECT name || '-grabbed', $1, $2, provider, base_url, api_key_encrypted, model, config, FALSE, NOW(), NOW()
    FROM llms
    WHERE llm_id = $3 AND is_public = TRUE
    RETURNING llm_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, is_public, created_at, updated_at
"#;

#[tracing::instrument(name = "database.get_llm", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", username = %user.as_str(), llm_id = %llm_id))]
pub(crate) async fn get_llm(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    llm_id: i32,
    encryption: &EncryptionService,
) -> Result<LargeLanguageModel> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GET_LLM_QUERY)
        .bind(llm_id)
        .bind(user.as_owner())
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    let llm = result?;
    decrypt_llm_api_key(encryption, llm)
}

/// Get LLM by ID using a pre-hashed owner_id directly.
/// Use this when the caller already has the hashed owner_id (e.g., from a transform record)
/// to avoid double-hashing through AuthenticatedUser.as_owner().
#[tracing::instrument(name = "database.get_llm_by_owner_id", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id, llm_id = %llm_id))]
pub(crate) async fn get_llm_by_owner_id(
    pool: &Pool<Postgres>,
    owner_id: &str,
    llm_id: i32,
    encryption: &EncryptionService,
) -> Result<LargeLanguageModel> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GET_LLM_QUERY)
        .bind(llm_id)
        .bind(owner_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    let llm = result?;
    decrypt_llm_api_key(encryption, llm)
}

#[tracing::instrument(name = "database.get_llms", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", username = %user.as_str()))]
pub(crate) async fn get_llms(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    limit: i64,
    offset: i64,
    encryption: &EncryptionService,
) -> Result<PaginatedResult<LargeLanguageModel>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LlmWithCount>(GET_LLMS_QUERY)
        .bind(user.as_owner())
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    let (llms, total_count) = LlmWithCount::into_parts(result?);
    let decrypted = decrypt_llms_api_keys(encryption, llms)?;

    Ok(PaginatedResult {
        items: decrypted,
        total_count,
        limit,
        offset,
    })
}

#[tracing::instrument(name = "database.get_llms_with_search", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", username = %user.as_str()))]
pub(crate) async fn get_llms_with_search(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    search_query: &str,
    limit: i64,
    offset: i64,
    encryption: &EncryptionService,
) -> Result<PaginatedResult<LargeLanguageModel>> {
    let search_pattern = format!("%{}%", search_query);

    let start = Instant::now();
    let result = sqlx::query_as::<_, LlmWithCount>(GET_LLMS_WITH_SEARCH_QUERY)
        .bind(user.as_owner())
        .bind(&search_pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    let (llms, total_count) = LlmWithCount::into_parts(result?);
    let decrypted = decrypt_llms_api_keys(encryption, llms)?;

    Ok(PaginatedResult {
        items: decrypted,
        total_count,
        limit,
        offset,
    })
}

#[tracing::instrument(name = "database.get_public_llms", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_public_llms(
    pool: &Pool<Postgres>,
    encryption: &EncryptionService,
) -> Result<Vec<LargeLanguageModel>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GET_PUBLIC_LLMS_QUERY)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    decrypt_llms_api_keys(encryption, result?)
}

#[tracing::instrument(name = "database.get_recent_public_llms", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_recent_public_llms(
    pool: &Pool<Postgres>,
    limit: i32,
    encryption: &EncryptionService,
) -> Result<Vec<LargeLanguageModel>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(GET_RECENT_PUBLIC_LLMS_QUERY)
        .bind(limit)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    decrypt_llms_api_keys(encryption, result?)
}

#[tracing::instrument(name = "database.create_llm", skip(pool, create_llm, encryption), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %user.as_owner()))]
pub(crate) async fn create_llm(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    create_llm: &CreateLLM,
    encryption: &EncryptionService,
) -> Result<LargeLanguageModel> {
    // Encrypt API key before storing
    let encrypted_api_key = encrypt_api_key(encryption, &create_llm.api_key)?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(CREATE_LLM_QUERY)
        .bind(&create_llm.name)
        .bind(user.as_owner())
        .bind(&**user)
        .bind(&create_llm.provider)
        .bind(&create_llm.base_url)
        .bind(&encrypted_api_key)
        .bind(&create_llm.model)
        .bind(&create_llm.config)
        .bind(create_llm.is_public)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "llms", duration, success);

    let llm = result?;
    // Decrypt api_key in response so it can be used immediately
    decrypt_llm_api_key(encryption, llm)
}

#[tracing::instrument(name = "database.delete_llm", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", username = %user.as_str(), llm_id = %llm_id))]
pub(crate) async fn delete_llm(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    llm_id: i32,
) -> Result<()> {
    let start = Instant::now();
    let result = sqlx::query(DELETE_LLM_QUERY)
        .bind(llm_id)
        .bind(user.as_owner())
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "llms", duration, success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.update_llm", skip(pool, update_llm, encryption), fields(database.system = "postgresql", database.operation = "UPDATE", username = %user.as_str(), llm_id = %llm_id))]
pub(crate) async fn update_llm(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    llm_id: i32,
    update_llm: &UpdateLargeLanguageModel,
    encryption: &EncryptionService,
) -> Result<LargeLanguageModel> {
    // Encrypt API key if provided
    let encrypted_api_key = encrypt_api_key(encryption, &update_llm.api_key)?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, LargeLanguageModel>(UPDATE_LLM_QUERY)
        .bind(llm_id)
        .bind(&update_llm.name)
        .bind(&update_llm.base_url)
        .bind(&encrypted_api_key)
        .bind(&update_llm.config)
        .bind(update_llm.is_public)
        .bind(user.as_owner())
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "llms", duration, success);

    let llm = result?;
    // Decrypt api_key in response
    decrypt_llm_api_key(encryption, llm)
}

#[tracing::instrument(name = "database.grab_public_llm", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %user.as_owner(), llm_id = %llm_id))]
pub(crate) async fn grab_public_llm(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    llm_id: i32,
    encryption: &EncryptionService,
) -> Result<LargeLanguageModel> {
    let start = Instant::now();
    // The encrypted key is copied from the source LLM to the new one
    let result = sqlx::query_as::<_, LargeLanguageModel>(GRAB_PUBLIC_LLM_QUERY)
        .bind(user.as_owner())
        .bind(&**user)
        .bind(llm_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "llms", duration, success);

    let llm = result?;
    // Decrypt api_key for the response
    decrypt_llm_api_key(encryption, llm)
}

/// Helper to encrypt an optional API key
fn encrypt_api_key(
    encryption: &EncryptionService,
    api_key: &Option<String>,
) -> Result<Option<String>> {
    match api_key {
        Some(key) if !key.is_empty() => Ok(Some(encryption.encrypt(key)?)),
        _ => Ok(None),
    }
}

/// Helper to decrypt api_key in a LargeLanguageModel
fn decrypt_llm_api_key(
    encryption: &EncryptionService,
    mut llm: LargeLanguageModel,
) -> Result<LargeLanguageModel> {
    if let Some(ref encrypted_key) = llm.api_key
        && !encrypted_key.is_empty()
    {
        llm.api_key = Some(encryption.decrypt(encrypted_key)?);
    }
    Ok(llm)
}

/// Helper to decrypt api_key in multiple LargeLanguageModels
fn decrypt_llms_api_keys(
    encryption: &EncryptionService,
    llms: Vec<LargeLanguageModel>,
) -> Result<Vec<LargeLanguageModel>> {
    llms.into_iter()
        .map(|llm| decrypt_llm_api_key(encryption, llm))
        .collect()
}
