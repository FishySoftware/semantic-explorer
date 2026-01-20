use crate::{auth::AuthenticatedUser, errors::ApiError};
use actix_web::{HttpResponse, Responder, ResponseError, get, web::Data};
use semantic_explorer_core::{config::LlmInferenceConfig, http_client::HTTP_CLIENT};

#[utoipa::path(
    get,
    path = "/api/llm-inference/models",
    tag = "LLM Inference",
    responses(
        (status = 200, description = "List of available LLM models from the llm inference api", body = Vec<String>),
        (status = 500, description = "Failed to fetch models"),
    ),
)]
#[get("/api/llm-inference/models")]
#[tracing::instrument(name = "list_llm_inference_models", skip(_user, llm_inference_config))]
pub(crate) async fn list_inference_llms(
    _user: AuthenticatedUser,
    llm_inference_config: Data<LlmInferenceConfig>,
) -> impl Responder {
    let url = format!(
        "{}/api/llms",
        llm_inference_config.url.trim_end_matches('/')
    );

    match HTTP_CLIENT.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                return ApiError::Internal(format!(
                    "HTTP {}: check llm-inference-api is running",
                    status
                ))
                .error_response();
            }

            match response.json::<serde_json::Value>().await {
                Ok(models) => {
                    // Return models array directly (array of ModelInfo objects)
                    HttpResponse::Ok().json(serde_json::json!({ "models": models }))
                }
                Err(e) => ApiError::Internal(format!("Failed to parse LLM models: {}", e))
                    .error_response(),
            }
        }
        Err(e) => ApiError::Internal(format!("Failed to connect to llm-inference-api: {}", e))
            .error_response(),
    }
}
