use anyhow::Result;
use sqlx::{FromRow, Pool, Postgres};
use std::time::Instant;

use crate::auth::AuthenticatedUser;
use crate::embedders::models::{CreateEmbedder, Embedder, UpdateEmbedder};
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::observability::record_database_query;

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

/// Helper to decrypt api_key in an Embedder
fn decrypt_embedder_api_key(
    encryption: &EncryptionService,
    mut embedder: Embedder,
) -> Result<Embedder> {
    if let Some(ref encrypted_key) = embedder.api_key
        && !encrypted_key.is_empty()
    {
        embedder.api_key = Some(encryption.decrypt(encrypted_key)?);
    }
    Ok(embedder)
}

/// Helper to decrypt api_key in multiple Embedders
fn decrypt_embedders_api_keys(
    encryption: &EncryptionService,
    embedders: Vec<Embedder>,
) -> Result<Vec<Embedder>> {
    embedders
        .into_iter()
        .map(|e| decrypt_embedder_api_key(encryption, e))
        .collect()
}

#[derive(FromRow)]
pub struct EmbedderConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key_encrypted: Option<String>,
    pub config: serde_json::Value,
    pub dimensions: i32,
}

const GET_EMBEDDER_QUERY: &str = r#"
    SELECT embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at
    FROM embedders
    WHERE embedder_id = $1
"#;

const GET_EMBEDDERS_QUERY: &str = r#"
    SELECT embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at
    FROM embedders
    ORDER BY created_at DESC
"#;

const CREATE_EMBEDDER_QUERY: &str = r#"
    INSERT INTO embedders (name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NOW(), NOW())
    RETURNING embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at
"#;

const DELETE_EMBEDDER_QUERY: &str = r#"
    DELETE FROM embedders WHERE embedder_id = $1
"#;

const GET_PUBLIC_EMBEDDERS_QUERY: &str = r#"
    SELECT embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at
    FROM embedders
    WHERE is_public = TRUE
    ORDER BY created_at DESC
"#;

const GET_RECENT_PUBLIC_EMBEDDERS_QUERY: &str = r#"
    SELECT embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at
    FROM embedders
    WHERE is_public = TRUE
    ORDER BY updated_at DESC
    LIMIT $1
"#;

const GRAB_PUBLIC_EMBEDDER_QUERY: &str = r#"
    INSERT INTO embedders (name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at)
    SELECT name || ' - grabbed', $1, $2, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, NULL, FALSE, NOW(), NOW()
    FROM embedders
    WHERE embedder_id = $3 AND is_public = TRUE
    RETURNING embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at
"#;

const UPDATE_EMBEDDER_QUERY: &str = r#"
    UPDATE embedders
    SET name = COALESCE($2, name),
        base_url = COALESCE($3, base_url),
        api_key_encrypted = COALESCE($4, api_key_encrypted),
        config = COALESCE($5, config),
        batch_size = COALESCE($6, batch_size),
        max_batch_size = COALESCE($7, max_batch_size),
        dimensions = COALESCE($8, dimensions),
        max_input_tokens = COALESCE($9, max_input_tokens),
        truncate_strategy = COALESCE($10, truncate_strategy),
        collection_name = COALESCE($11, collection_name),
        is_public = COALESCE($12, is_public),
        updated_at = NOW()
    WHERE embedder_id = $1
    RETURNING embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size, max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name, is_public, created_at, updated_at
"#;

const GET_EMBEDDER_BY_ID_QUERY: &str = r#"
    SELECT provider, base_url, api_key_encrypted, config, dimensions
    FROM embedders
    WHERE embedder_id = $1
"#;

const GET_EMBEDDERS_BATCH: &str = r#"
        SELECT embedder_id, name, owner_id, owner_display_name, provider, base_url, api_key_encrypted, config, batch_size,
               max_batch_size, dimensions, max_input_tokens, truncate_strategy, collection_name,
               is_public, created_at, updated_at
        FROM embedders
        WHERE embedder_id = ANY($1)
        ORDER BY embedder_id
    "#;

#[tracing::instrument(name = "database.get_embedder", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", username = %user.as_str(), embedder_id = %embedder_id))]
pub(crate) async fn get_embedder(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    embedder_id: i32,
    encryption: &EncryptionService,
) -> Result<Embedder> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, user.as_str()).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(GET_EMBEDDER_QUERY)
        .bind(embedder_id)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    let embedder = result?;
    tx.commit().await?;
    decrypt_embedder_api_key(encryption, embedder)
}

/// Batch fetch embedders (avoids N+1 queries)
#[tracing::instrument(name = "database.get_embedders_batch", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", username = %user.as_str()))]
pub(crate) async fn get_embedders_batch(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    embedder_ids: &[i32],
    encryption: &EncryptionService,
) -> Result<Vec<Embedder>> {
    if embedder_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, user.as_str()).await?;

    let start = Instant::now();

    let result = sqlx::query_as::<_, Embedder>(GET_EMBEDDERS_BATCH)
        .bind(embedder_ids.to_vec())
        .fetch_all(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    let embedders = result?;
    tx.commit().await?;
    decrypt_embedders_api_keys(encryption, embedders)
}

#[tracing::instrument(name = "database.get_embedders", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", username = %user.as_str()))]
pub(crate) async fn get_embedders(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    encryption: &EncryptionService,
) -> Result<Vec<Embedder>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, user.as_str()).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(GET_EMBEDDERS_QUERY)
        .fetch_all(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    let embedders = result?;
    tx.commit().await?;
    decrypt_embedders_api_keys(encryption, embedders)
}

#[tracing::instrument(name = "database.create_embedder", skip(pool, create_embedder, encryption), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %user.username()))]
pub(crate) async fn create_embedder(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    create_embedder: &CreateEmbedder,
    encryption: &EncryptionService,
) -> Result<Embedder> {
    // Encrypt API key before storing
    let encrypted_api_key = encrypt_api_key(encryption, &create_embedder.api_key)?;

    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, user.as_str()).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(CREATE_EMBEDDER_QUERY)
        .bind(&create_embedder.name)
        .bind(user.as_str())
        .bind(user.username())
        .bind(&create_embedder.provider)
        .bind(&create_embedder.base_url)
        .bind(&encrypted_api_key)
        .bind(&create_embedder.config)
        .bind(create_embedder.batch_size)
        .bind(create_embedder.max_batch_size)
        .bind(create_embedder.dimensions)
        .bind(create_embedder.max_input_tokens)
        .bind(&create_embedder.truncate_strategy)
        .bind(&create_embedder.collection_name)
        .bind(create_embedder.is_public)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "embedders", duration, success);

    let embedder = result?;
    tx.commit().await?;
    // Decrypt api_key in response so it can be used immediately
    decrypt_embedder_api_key(encryption, embedder)
}

#[tracing::instrument(name = "database.delete_embedder", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", username = %user.as_str(), embedder_id = %embedder_id))]
pub(crate) async fn delete_embedder(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    embedder_id: i32,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, user.as_str()).await?;

    let start = Instant::now();
    let result = sqlx::query(DELETE_EMBEDDER_QUERY)
        .bind(embedder_id)
        .execute(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "embedders", duration, success);

    result?;
    tx.commit().await?;
    Ok(())
}

#[tracing::instrument(name = "database.update_embedder", skip(pool, update_embedder, encryption), fields(database.system = "postgresql", database.operation = "UPDATE", username = %user.as_str(), embedder_id = %embedder_id))]
pub(crate) async fn update_embedder(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    embedder_id: i32,
    update_embedder: &UpdateEmbedder,
    encryption: &EncryptionService,
) -> Result<Embedder> {
    // Encrypt API key if provided
    let encrypted_api_key = encrypt_api_key(encryption, &update_embedder.api_key)?;

    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, user.as_str()).await?;

    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(UPDATE_EMBEDDER_QUERY)
        .bind(embedder_id)
        .bind(&update_embedder.name)
        .bind(&update_embedder.base_url)
        .bind(&encrypted_api_key)
        .bind(&update_embedder.config)
        .bind(update_embedder.batch_size)
        .bind(update_embedder.max_batch_size)
        .bind(update_embedder.dimensions)
        .bind(update_embedder.max_input_tokens)
        .bind(&update_embedder.truncate_strategy)
        .bind(&update_embedder.collection_name)
        .bind(update_embedder.is_public)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("UPDATE", "embedders", duration, success);

    let embedder = result?;
    tx.commit().await?;
    // Decrypt api_key in response
    decrypt_embedder_api_key(encryption, embedder)
}

#[tracing::instrument(name = "database.get_public_embedders", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_public_embedders(
    pool: &Pool<Postgres>,
    encryption: &EncryptionService,
) -> Result<Vec<Embedder>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(GET_PUBLIC_EMBEDDERS_QUERY)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    decrypt_embedders_api_keys(encryption, result?)
}

#[tracing::instrument(name = "database.get_recent_public_embedders", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_recent_public_embedders(
    pool: &Pool<Postgres>,
    limit: i32,
    encryption: &EncryptionService,
) -> Result<Vec<Embedder>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, Embedder>(GET_RECENT_PUBLIC_EMBEDDERS_QUERY)
        .bind(limit)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    decrypt_embedders_api_keys(encryption, result?)
}

#[tracing::instrument(name = "database.grab_public_embedder", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %user.username(), embedder_id = %embedder_id))]
pub(crate) async fn grab_public_embedder(
    pool: &Pool<Postgres>,
    user: &AuthenticatedUser,
    embedder_id: i32,
    encryption: &EncryptionService,
) -> Result<Embedder> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, user.as_str()).await?;

    let start = Instant::now();
    // The encrypted key is copied from the source embedder to the new one
    let result = sqlx::query_as::<_, Embedder>(GRAB_PUBLIC_EMBEDDER_QUERY)
        .bind(user.as_str())
        .bind(user.username())
        .bind(embedder_id)
        .fetch_one(&mut *tx)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "embedders", duration, success);

    let embedder = result?;
    tx.commit().await?;
    // Decrypt api_key for the response
    decrypt_embedder_api_key(encryption, embedder)
}

#[tracing::instrument(name = "database.get_embedder_config", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT", embedder_id = %embedder_id))]
pub async fn get_embedder_config(
    pool: &Pool<Postgres>,
    embedder_id: i32,
    encryption: &EncryptionService,
) -> Result<EmbedderConfig> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, EmbedderConfig>(GET_EMBEDDER_BY_ID_QUERY)
        .bind(embedder_id)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "embedders", duration, success);

    // Decrypt api_key for use
    let mut config = result?;
    if let Some(ref encrypted_key) = config.api_key_encrypted
        && !encrypted_key.is_empty()
    {
        config.api_key_encrypted = Some(encryption.decrypt(encrypted_key)?);
    }
    Ok(config)
}
