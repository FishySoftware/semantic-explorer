use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::{Pool, Postgres};
use tracing::{debug, info};
use uuid::Uuid;

const UPDATE_SESSION_ACTIVITY_QUERY: &str = r#"
    WITH updated AS (
        UPDATE user_sessions
        SET last_activity_at = NOW()
        WHERE username = $1
          AND revoked_at IS NULL
          AND expires_at > NOW()
          AND ($2::text IS NULL OR ip_address = $2)
          AND ($3::text IS NULL OR user_agent = $3)
        RETURNING 1
    )
    SELECT COUNT(*) FROM updated
"#;

const COUNT_ACTIVE_SESSIONS_QUERY: &str = r#"
    SELECT COUNT(*) FROM user_sessions
    WHERE username = $1
      AND revoked_at IS NULL
      AND expires_at > NOW()
"#;

const UPDATE_ANY_SESSION_ACTIVITY_QUERY: &str = r#"
    UPDATE user_sessions
    SET last_activity_at = NOW()
    WHERE username = $1
      AND revoked_at IS NULL
      AND expires_at > NOW()
"#;

const INSERT_SESSION_QUERY: &str = r#"
    INSERT INTO user_sessions (
        session_id,
        username,
        id_token_hash,
        access_token_hash,
        expires_at,
        ip_address,
        user_agent,
        created_at,
        last_activity_at
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
    ON CONFLICT (session_id) DO NOTHING
"#;

// ================================
// CONSTANTS
// ================================

/// Default session expiry in hours
pub const DEFAULT_SESSION_EXPIRY_HOURS: i64 = 24 * 7; // 7 days

// ================================
// PUBLIC FUNCTIONS
// ================================

/// Ensures a session exists for the user and updates its activity timestamp.
/// Creates a new session if none exists or if all existing sessions have expired.
pub async fn ensure_session_and_update_activity(
    pool: &Pool<Postgres>,
    username: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(), anyhow::Error> {
    // Try to update existing session first (most common case)
    let updated = sqlx::query_scalar::<_, i64>(UPDATE_SESSION_ACTIVITY_QUERY)
        .bind(username)
        .bind(ip_address)
        .bind(user_agent)
        .fetch_one(pool)
        .await?;

    if updated > 0 {
        debug!(username = %username, "Updated session activity");
        return Ok(());
    }

    // Check if we should create a new session or update any existing session for user
    let existing_count = sqlx::query_scalar::<_, i64>(COUNT_ACTIVE_SESSIONS_QUERY)
        .bind(username)
        .fetch_one(pool)
        .await?;

    if existing_count > 0 {
        // User has sessions, but not from this device - update the most recent one
        sqlx::query(UPDATE_ANY_SESSION_ACTIVITY_QUERY)
            .bind(username)
            .execute(pool)
            .await?;
        debug!(username = %username, "Updated activity for existing sessions");
    } else {
        // No active sessions - create a new one
        create_session(pool, username, ip_address, user_agent).await?;
    }

    Ok(())
}

/// Creates a new session for a user.
/// Uses placeholder token hashes since OIDC tokens are managed by the OIDC middleware.
pub async fn create_session(
    pool: &Pool<Postgres>,
    username: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(), anyhow::Error> {
    let session_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::hours(DEFAULT_SESSION_EXPIRY_HOURS);

    // Create placeholder token hashes (since we don't have access to actual OIDC tokens)
    // These are derived from session_id to ensure uniqueness
    let token_placeholder = format!("session:{}:{}", session_id, now.timestamp());
    let token_hash = generate_token_hash(&token_placeholder);

    sqlx::query(INSERT_SESSION_QUERY)
        .bind(&session_id)
        .bind(username)
        .bind(&token_hash)
        .bind(&token_hash)
        .bind(expires_at)
        .bind(ip_address)
        .bind(user_agent)
        .execute(pool)
        .await?;

    info!(
        username = %username,
        session_id = %session_id,
        expires_at = %expires_at,
        "Created new session for authenticated user"
    );

    Ok(())
}

/// Generates a SHA-256 hash for token storage.
fn generate_token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
