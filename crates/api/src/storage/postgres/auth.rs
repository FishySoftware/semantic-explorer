use anyhow::Result;
use semantic_explorer_core::observability::record_database_query;
use sqlx::{Pool, Postgres, Transaction};
use std::time::Instant;

// SQL queries as constants
const SQL_LOG_SESSION_EVENT: &str = r#"
    INSERT INTO session_events (session_id, event_type, reason, metadata)
    VALUES ($1, $2, $3, $4)
"#;

const SQL_GET_SESSION: &str = r#"
    SELECT session_id, username, id_token_hash, access_token_hash,
           refresh_token_hash, created_at, last_activity_at,
           expires_at, revoked_at, revoked_reason, ip_address, user_agent
    FROM user_sessions
    WHERE session_id = $1
"#;

const SQL_GET_USER_SESSIONS: &str = r#"
    SELECT session_id, username, id_token_hash, access_token_hash,
           refresh_token_hash, created_at, last_activity_at,
           expires_at, revoked_at, revoked_reason, ip_address, user_agent
    FROM user_sessions
    WHERE username = $1
      AND revoked_at IS NULL
      AND expires_at > NOW()
    ORDER BY last_activity_at DESC
"#;

const SQL_REVOKE_SESSION: &str = r#"
    UPDATE user_sessions
    SET revoked_at = NOW()
    WHERE session_id = $1
      AND revoked_at IS NULL
    RETURNING session_id
"#;

const SQL_REVOKE_ALL_USER_SESSIONS: &str = r#"
    UPDATE user_sessions
    SET revoked_at = NOW()
    WHERE username = $1
      AND revoked_at IS NULL
"#;

const SQL_CLEANUP_EXPIRED_SESSIONS: &str = r#"
    UPDATE user_sessions
    SET revoked_at = NOW()
    WHERE expires_at < NOW()
      AND revoked_at IS NULL
"#;

// Database structs matching query results
// Note: Some fields are only used by sqlx for deserialization from DB rows
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct UserSession {
    pub session_id: String,
    pub username: String,
    pub id_token_hash: String,
    pub access_token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_reason: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Log a session event
pub async fn log_session_event(
    tx: &mut Transaction<'_, Postgres>,
    session_id: &str,
    event_type: &str,
    reason: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<()> {
    let start = Instant::now();

    sqlx::query(SQL_LOG_SESSION_EVENT)
        .bind(session_id)
        .bind(event_type)
        .bind(reason)
        .bind(metadata)
        .execute(&mut **tx)
        .await?;

    record_database_query(
        "INSERT",
        "session_events",
        start.elapsed().as_secs_f64(),
        true,
    );
    Ok(())
}

/// Get a specific session by ID
pub async fn get_session(pool: &Pool<Postgres>, session_id: &str) -> Result<Option<UserSession>> {
    let start = Instant::now();

    let session = sqlx::query_as::<_, UserSession>(SQL_GET_SESSION)
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

    record_database_query(
        "SELECT",
        "user_sessions",
        start.elapsed().as_secs_f64(),
        true,
    );
    Ok(session)
}

/// Get all active sessions for a user
pub async fn get_user_sessions(pool: &Pool<Postgres>, username: &str) -> Result<Vec<UserSession>> {
    let start = Instant::now();

    let sessions = sqlx::query_as::<_, UserSession>(SQL_GET_USER_SESSIONS)
        .bind(username)
        .fetch_all(pool)
        .await?;

    record_database_query(
        "SELECT",
        "user_sessions",
        start.elapsed().as_secs_f64(),
        true,
    );
    Ok(sessions)
}

/// Revoke a single session
pub async fn revoke_session(pool: &Pool<Postgres>, session_id: &str) -> Result<bool> {
    let start = Instant::now();

    let result = sqlx::query(SQL_REVOKE_SESSION)
        .bind(session_id)
        .execute(pool)
        .await?;

    record_database_query(
        "UPDATE",
        "user_sessions",
        start.elapsed().as_secs_f64(),
        true,
    );
    Ok(result.rows_affected() > 0)
}

/// Revoke all sessions for a user
pub async fn revoke_all_user_sessions(pool: &Pool<Postgres>, username: &str) -> Result<u64> {
    let start = Instant::now();

    let result = sqlx::query(SQL_REVOKE_ALL_USER_SESSIONS)
        .bind(username)
        .execute(pool)
        .await?;

    record_database_query(
        "UPDATE",
        "user_sessions",
        start.elapsed().as_secs_f64(),
        true,
    );
    Ok(result.rows_affected())
}

/// Cleanup expired sessions
pub async fn cleanup_expired_sessions(pool: &Pool<Postgres>) -> Result<u64> {
    let start = Instant::now();

    let result = sqlx::query(SQL_CLEANUP_EXPIRED_SESSIONS)
        .execute(pool)
        .await?;

    record_database_query(
        "UPDATE",
        "user_sessions",
        start.elapsed().as_secs_f64(),
        true,
    );
    Ok(result.rows_affected())
}
