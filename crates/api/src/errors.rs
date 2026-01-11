use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde_json::json;
use thiserror::Error;

/// Unified API error type for consistent error handling across all endpoints.
///
/// This enum provides structured error responses with proper HTTP status codes
/// and consistent JSON formatting.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Resource not found (404)
    #[error("{0}")]
    NotFound(String),
    /// Bad request / validation error (400)
    #[error("{0}")]
    BadRequest(String),
    /// Unauthorized access (401) - Ready for Phase 5.2 migration
    #[error("{0}")]
    Unauthorized(String),
    /// Internal server error (500)
    #[error("{0}")]
    Internal(String),
    /// Database error (500)
    #[error("Database error: {0}")]
    Database(sqlx::Error),
    /// Validation error from input validation
    #[error(transparent)]
    Validation(#[from] semantic_explorer_core::validation::ValidationError),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Validation(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        HttpResponse::build(status).json(json!({
            "error": self.to_string(),
            "status": status.as_u16()
        }))
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::RowNotFound => ApiError::NotFound("Resource not found".to_string()),
            _ => ApiError::Database(e),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

// ============================================================================
// Legacy helper functions for backward compatibility during migration
// These can be removed once all handlers are migrated to use ApiError
// ============================================================================

/// Create a standardized JSON error response
pub(crate) fn error_response(
    status: actix_web::http::StatusCode,
    message: impl std::fmt::Display,
) -> HttpResponse {
    HttpResponse::build(status).json(json!({
        "error": message.to_string()
    }))
}

/// Create a Bad Request JSON response
pub(crate) fn bad_request(message: impl std::fmt::Display) -> HttpResponse {
    error_response(actix_web::http::StatusCode::BAD_REQUEST, message)
}

/// Create a Not Found JSON response
pub(crate) fn not_found(message: impl std::fmt::Display) -> HttpResponse {
    error_response(actix_web::http::StatusCode::NOT_FOUND, message)
}

/// Create an Unauthorized JSON response
pub(crate) fn unauthorized(message: impl std::fmt::Display) -> HttpResponse {
    error_response(actix_web::http::StatusCode::UNAUTHORIZED, message)
}
