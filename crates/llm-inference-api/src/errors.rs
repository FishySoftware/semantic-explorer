//! Error types for the LLM inference API.

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use std::fmt;
use utoipa::ToResponse;

/// LLM Inference API errors
#[derive(Debug, ToResponse)]
pub enum InferenceError {
    /// Model loading failed
    ModelLoad(String),
    /// Text generation failed
    Generation(String),
    /// Unsupported model
    UnsupportedModel(String),
    /// Bad request
    BadRequest(String),
    /// Internal error
    Internal(String),
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InferenceError::ModelLoad(msg) => write!(f, "Failed to load model: {}", msg),
            InferenceError::Generation(msg) => write!(f, "Text generation failed: {}", msg),
            InferenceError::UnsupportedModel(msg) => write!(f, "Unsupported model: {}", msg),
            InferenceError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            InferenceError::Internal(msg) => write!(f, "Internal error: {}", msg),
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
            InferenceError::Generation(_) => StatusCode::INTERNAL_SERVER_ERROR,
            InferenceError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let code = match self {
            InferenceError::ModelLoad(_) => "MODEL_LOAD_ERROR",
            InferenceError::Generation(_) => "GENERATION_ERROR",
            InferenceError::UnsupportedModel(_) => "UNSUPPORTED_MODEL",
            InferenceError::BadRequest(_) => "BAD_REQUEST",
            InferenceError::Internal(_) => "INTERNAL_ERROR",
        };

        HttpResponse::build(self.status_code()).json(ErrorResponse {
            error: self.to_string(),
            code: code.to_string(),
        })
    }
}
