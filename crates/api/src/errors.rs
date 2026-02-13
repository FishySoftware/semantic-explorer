use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

/// Standardized API error response structure
///
/// This format is consistent across all API endpoints and includes:
/// - error: The error type/category
/// - message: User-friendly error message
/// - details: Optional structured details (not exposed in production)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ErrorResponse {
    /// Create a new error response with basic information
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: None,
        }
    }
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

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
    /// Conflict - resource has dependencies (409)
    #[error("{0}")]
    Conflict(String),
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
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Validation(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();

        let (error_type, message) = match self {
            ApiError::NotFound(msg) => ("NotFound", msg.clone()),
            ApiError::BadRequest(msg) => ("BadRequest", msg.clone()),
            ApiError::Unauthorized(msg) => ("Unauthorized", msg.clone()),
            ApiError::Conflict(msg) => ("Conflict", msg.clone()),
            ApiError::Internal(msg) => ("InternalServerError", msg.clone()),
            ApiError::Database(e) => {
                // Log the actual error for debugging but don't expose to client
                tracing::error!(error = %e, "Database error occurred");
                (
                    "DatabaseError",
                    "An internal database error occurred".to_string(),
                )
            }
            ApiError::Validation(e) => ("ValidationError", e.to_string()),
        };

        let response = ErrorResponse::new(error_type, message);

        HttpResponse::build(status).json(json!({
            "error": response.error,
            "message": response.message,
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

/// Create a standardized JSON error response with all standard fields
pub(crate) fn error_response_with_status(
    status: actix_web::http::StatusCode,
    error_type: impl Into<String>,
    message: impl std::fmt::Display,
) -> HttpResponse {
    let error_type = error_type.into();
    let message = message.to_string();

    HttpResponse::build(status).json(json!({
        "error": error_type,
        "message": message,
        "status": status.as_u16()
    }))
}

/// Create a Bad Request (400) JSON response with standardized format
pub(crate) fn bad_request(message: impl std::fmt::Display) -> HttpResponse {
    error_response_with_status(
        actix_web::http::StatusCode::BAD_REQUEST,
        "BadRequest",
        message,
    )
}

/// Create a Not Found (404) JSON response with standardized format
pub(crate) fn not_found(message: impl std::fmt::Display) -> HttpResponse {
    error_response_with_status(actix_web::http::StatusCode::NOT_FOUND, "NotFound", message)
}

/// Create an Unauthorized (401) JSON response with standardized format
pub(crate) fn unauthorized(message: impl std::fmt::Display) -> HttpResponse {
    error_response_with_status(
        actix_web::http::StatusCode::UNAUTHORIZED,
        "Unauthorized",
        message,
    )
}
