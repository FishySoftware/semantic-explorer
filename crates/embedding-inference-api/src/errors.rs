//! Error types for the inference API.

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use std::fmt;

/// Inference API errors
#[derive(Debug)]
pub enum InferenceError {
    /// Model loading failed
    ModelLoad(String),
    /// Embedding generation failed
    Embedding(String),
    /// Reranking failed
    Rerank(String),
    /// Unsupported model
    UnsupportedModel(String),
    /// Bad request
    BadRequest(String),
    /// Internal error
    Internal(String),
    /// Service unavailable (backpressure / memory pressure)
    ServiceUnavailable(String),
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InferenceError::ModelLoad(msg) => write!(f, "Failed to load model: {}", msg),
            InferenceError::Embedding(msg) => write!(f, "Embedding generation failed: {}", msg),
            InferenceError::Rerank(msg) => write!(f, "Reranking failed: {}", msg),
            InferenceError::UnsupportedModel(msg) => write!(f, "Unsupported model: {}", msg),
            InferenceError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            InferenceError::Internal(msg) => write!(f, "Internal error: {}", msg),
            InferenceError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {}", msg),
        }
    }
}

impl std::error::Error for InferenceError {}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
}

impl ResponseError for InferenceError {
    fn status_code(&self) -> StatusCode {
        match self {
            InferenceError::BadRequest(_) => StatusCode::BAD_REQUEST,
            InferenceError::UnsupportedModel(_) => StatusCode::BAD_REQUEST,
            InferenceError::ModelLoad(_) => StatusCode::SERVICE_UNAVAILABLE,
            InferenceError::Embedding(_) => StatusCode::INTERNAL_SERVER_ERROR,
            InferenceError::Rerank(_) => StatusCode::INTERNAL_SERVER_ERROR,
            InferenceError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            InferenceError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let code = match self {
            InferenceError::ModelLoad(_) => "MODEL_LOAD_ERROR",
            InferenceError::Embedding(_) => "EMBEDDING_ERROR",
            InferenceError::Rerank(_) => "RERANK_ERROR",
            InferenceError::UnsupportedModel(_) => "UNSUPPORTED_MODEL",
            InferenceError::BadRequest(_) => "BAD_REQUEST",
            InferenceError::Internal(_) => "INTERNAL_ERROR",
            InferenceError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
        };

        let mut response = HttpResponse::build(self.status_code());

        // Include Retry-After header on 503 so clients can pace themselves
        if matches!(
            self,
            InferenceError::ServiceUnavailable(_) | InferenceError::ModelLoad(_)
        ) {
            response.insert_header(("Retry-After", "5"));
        }

        response.json(ErrorResponse {
            error: self.to_string(),
            code: code.to_string(),
        })
    }
}
