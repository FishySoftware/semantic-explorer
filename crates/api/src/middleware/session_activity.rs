//! Session activity tracking middleware
//!
//! Tracks user sessions and updates the last_activity timestamp for authenticated requests.
//! Sessions are created on-demand when an authenticated request is made without an existing session.
//! This middleware should be placed after OIDC authentication in the middleware stack.

use crate::auth::AuthenticatedUser;
use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
};
use chrono::{Duration, Utc};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use sha2::{Digest, Sha256};
use sqlx::{Pool, Postgres};
use std::{
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Default session expiry in hours
const DEFAULT_SESSION_EXPIRY_HOURS: i64 = 24 * 7; // 7 days

/// Session activity tracking middleware
pub struct SessionActivityMiddleware {
    pool: Arc<Pool<Postgres>>,
}

impl SessionActivityMiddleware {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for SessionActivityMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SessionActivityService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SessionActivityService {
            service: Rc::new(service),
            pool: self.pool.clone(),
        }))
    }
}

pub struct SessionActivityService<S> {
    service: Rc<S>,
    pool: Arc<Pool<Postgres>>,
}

impl<S, B> Service<ServiceRequest> for SessionActivityService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let pool = self.pool.clone();

        // Extract request metadata before processing
        let username_opt = req
            .extensions()
            .get::<AuthenticatedUser>()
            .map(|user| user.0.clone());

        let ip_address = req.connection_info().peer_addr().map(|s| s.to_string());

        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        Box::pin(async move {
            // Process the request first
            let res = service.call(req).await?;

            // Track session activity in background (fire-and-forget)
            if let Some(username) = username_opt {
                let pool_clone = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) = ensure_session_and_update_activity(
                        &pool_clone,
                        &username,
                        ip_address.as_deref(),
                        user_agent.as_deref(),
                    )
                    .await
                    {
                        warn!(error = %e, username = %username, "Failed to track session activity");
                    }
                });
            }

            Ok(res)
        })
    }
}

/// Ensure a session exists for the user and update its activity timestamp.
/// Creates a new session if none exists or if all existing sessions have expired.
async fn ensure_session_and_update_activity(
    pool: &Pool<Postgres>,
    username: &str,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
) -> Result<(), anyhow::Error> {
    // Try to update existing session first (most common case)
    let updated = sqlx::query_scalar::<_, i64>(
        r#"
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
        "#,
    )
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
    let existing_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM user_sessions
        WHERE username = $1
          AND revoked_at IS NULL
          AND expires_at > NOW()
        "#,
    )
    .bind(username)
    .fetch_one(pool)
    .await?;

    if existing_count > 0 {
        // User has sessions, but not from this device - update the most recent one
        sqlx::query(
            r#"
            UPDATE user_sessions
            SET last_activity_at = NOW()
            WHERE username = $1
              AND revoked_at IS NULL
              AND expires_at > NOW()
            "#,
        )
        .bind(username)
        .execute(pool)
        .await?;
        debug!(username = %username, "Updated activity for existing sessions");
    } else {
        // No active sessions - create a new one
        create_session_for_user(pool, username, ip_address, user_agent).await?;
    }

    Ok(())
}

/// Create a new session for a user.
/// Uses placeholder token hashes since OIDC tokens are managed by the OIDC middleware.
async fn create_session_for_user(
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
    let mut hasher = Sha256::new();
    hasher.update(token_placeholder.as_bytes());
    let token_hash = format!("{:x}", hasher.finalize());

    sqlx::query(
        r#"
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
        "#,
    )
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
