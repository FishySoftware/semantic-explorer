use actix_web::HttpResponse;
use serde_json::json;

/// Create a standardized JSON error response
pub(crate) fn error_response(
    status: actix_web::http::StatusCode,
    message: impl std::fmt::Display,
) -> HttpResponse {
    HttpResponse::build(status).json(json!({
        "error": message.to_string()
    }))
}

/// Create an Internal Server Error JSON response
pub(crate) fn internal_error(message: impl std::fmt::Display) -> HttpResponse {
    error_response(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, message)
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
