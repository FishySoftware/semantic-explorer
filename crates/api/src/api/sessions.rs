//! Session management API endpoints
//!
//! Provides endpoints for users to manage their active OIDC sessions:
//! - List all active sessions
//! - Revoke a specific session
//! - Revoke all sessions (logout everywhere)

use crate::auth::{AuthenticatedUser, session_manager, session_manager::UserSession};
use actix_web::{HttpResponse, delete, get, web};
use semantic_explorer_core::observability::record_database_query;
use sqlx::{Pool, Postgres};
use std::time::Instant;
use tracing::{error, info};
use utoipa::ToSchema;

/// Response for listing active sessions
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Session information exposed to the user
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct SessionInfo {
    pub session_id: String,
    pub created_at: i64,
    pub last_activity: i64,
    pub expires_at: i64,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub is_current: bool,
}

impl From<UserSession> for SessionInfo {
    fn from(session: UserSession) -> Self {
        Self {
            session_id: session.session_id,
            created_at: session.created_at.timestamp(),
            last_activity: session.last_activity_at.timestamp(),
            expires_at: session.expires_at.timestamp(),
            ip_address: session.ip_address,
            user_agent: session.user_agent,
            is_current: false, // Will be set by caller
        }
    }
}

/// Get all active sessions for the authenticated user
#[utoipa::path(
    get,
    path = "/api/auth/sessions",
    tag = "Authentication",
    responses(
        (status = 200, description = "List of active sessions", body = SessionListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("oidc" = [])
    )
)]
#[get("/api/auth/sessions")]
pub async fn list_sessions(
    pool: web::Data<Pool<Postgres>>,
    user: AuthenticatedUser,
) -> impl actix_web::Responder {
    let start = Instant::now();
    let username = &user.0;

    match session_manager::get_user_sessions(&pool, username).await {
        Ok(sessions) => {
            let total = sessions.len();
            let sessions: Vec<SessionInfo> = sessions.into_iter().map(SessionInfo::from).collect();

            let duration = start.elapsed().as_secs_f64();
            record_database_query("SELECT", "user_sessions", duration, true);
            info!(
                username = %username,
                total = %total,
                "Listed active sessions"
            );

            HttpResponse::Ok().json(SessionListResponse { sessions, total })
        }
        Err(e) => {
            error!(error = %e, username = %username, "Failed to list sessions");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to retrieve sessions"
            }))
        }
    }
}

/// Revoke a specific session by ID
#[utoipa::path(
    delete,
    path = "/api/auth/sessions/{session_id}",
    tag = "Authentication",
    params(
        ("session_id" = String, Path, description = "Session ID to revoke")
    ),
    responses(
        (status = 200, description = "Session revoked successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Cannot revoke another user's session"),
        (status = 404, description = "Session not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("oidc" = [])
    )
)]
#[delete("/api/auth/sessions/{session_id}")]
pub async fn revoke_session(
    pool: web::Data<Pool<Postgres>>,
    user: AuthenticatedUser,
    session_id: web::Path<String>,
) -> impl actix_web::Responder {
    let session_id = session_id.into_inner();

    // Verify the session belongs to the user
    match session_manager::get_session(&pool, &session_id).await {
        Ok(Some(session)) => {
            if session.username != user.0 {
                return HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "Cannot revoke another user's session"
                }));
            }

            // Revoke the session
            match session_manager::revoke_session(&pool, &session_id, "user_requested").await {
                Ok(_) => {
                    info!(
                        username = %user.0,
                        session_id = %session_id,
                        "Session revoked by user"
                    );

                    HttpResponse::Ok().json(serde_json::json!({
                        "message": "Session revoked successfully",
                        "session_id": session_id
                    }))
                }
                Err(e) => {
                    error!(
                        error = %e,
                        username = %user.0,
                        session_id = %session_id,
                        "Failed to revoke session"
                    );
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to revoke session"
                    }))
                }
            }
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Session not found"
        })),
        Err(e) => {
            error!(error = %e, session_id = %session_id, "Failed to get session");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to retrieve session"
            }))
        }
    }
}

/// Revoke all sessions for the authenticated user (logout everywhere)
#[utoipa::path(
    delete,
    path = "/api/auth/sessions",
    tag = "Authentication",
    responses(
        (status = 200, description = "All sessions revoked successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("oidc" = [])
    )
)]
#[delete("/api/auth/sessions")]
pub async fn revoke_all_sessions(
    pool: web::Data<Pool<Postgres>>,
    user: AuthenticatedUser,
) -> impl actix_web::Responder {
    match session_manager::revoke_all_user_sessions(&pool, &user.0, "user_requested_all").await {
        Ok(count) => {
            info!(
                username = %user.0,
                count = %count,
                "All sessions revoked by user"
            );

            HttpResponse::Ok().json(serde_json::json!({
                "message": "All sessions revoked successfully",
                "revoked_count": count
            }))
        }
        Err(e) => {
            error!(error = %e, username = %user.0, "Failed to revoke all sessions");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to revoke sessions"
            }))
        }
    }
}
