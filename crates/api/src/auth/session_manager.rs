use crate::storage::postgres::auth as auth_db;
use anyhow::{Result, anyhow};
use sqlx::{Pool, Postgres};
use tracing::{info, warn};

// Re-export UserSession from storage layer
pub use auth_db::UserSession;

/// Get a specific session by ID
#[tracing::instrument(name = "session.get", skip(pool), fields(session_id = %session_id))]
pub async fn get_session(pool: &Pool<Postgres>, session_id: &str) -> Result<Option<UserSession>> {
    auth_db::get_session(pool, session_id).await
}

/// Get all active sessions for a user
#[tracing::instrument(name = "session.get_user_sessions", skip(pool), fields(username = %username))]
pub async fn get_user_sessions(pool: &Pool<Postgres>, username: &str) -> Result<Vec<UserSession>> {
    auth_db::get_user_sessions(pool, username).await
}

/// Revoke a single session
#[tracing::instrument(name = "session.revoke", skip(pool), fields(session_id = %session_id))]
pub async fn revoke_session(pool: &Pool<Postgres>, session_id: &str, reason: &str) -> Result<()> {
    let revoked = auth_db::revoke_session(pool, session_id).await?;

    if !revoked {
        warn!(session_id = %session_id, "Attempted to revoke non-existent or already revoked session");
        return Err(anyhow!("Session not found or already revoked"));
    }

    // Log revocation event (best effort - don't fail if logging fails)
    let mut tx = pool.begin().await?;
    if let Err(e) =
        auth_db::log_session_event(&mut tx, session_id, "revoked", Some(reason), None).await
    {
        warn!(error = %e, "Failed to log session revocation event");
    } else {
        let _ = tx.commit().await;
    }

    info!(
        session_id = %session_id,
        reason = %reason,
        "Session revoked"
    );

    Ok(())
}

/// Revoke all sessions for a user
#[tracing::instrument(name = "session.revoke_all", skip(pool), fields(username = %username))]
pub async fn revoke_all_user_sessions(
    pool: &Pool<Postgres>,
    username: &str,
    reason: &str,
) -> Result<u64> {
    let count = auth_db::revoke_all_user_sessions(pool, username).await?;

    info!(
        username = %username,
        count = %count,
        reason = %reason,
        "Revoked all user sessions"
    );

    Ok(count)
}
