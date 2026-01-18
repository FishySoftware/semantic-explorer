use crate::{api::embedders::ModelInfo, auth::AuthenticatedUser, errors::ApiError};
use actix_web::{HttpResponse, Responder, ResponseError, get, web::Data};
use semantic_explorer_core::{config::EmbeddingInferenceConfig, http_client::HTTP_CLIENT};

#[utoipa::path(
    get,
    path = "/api/embedding-inference/models",
    tag = "Embedding Inference",
    responses(
        (status = 200, description = "List of available embedding models from the embedding inference api", body = Vec<ModelInfo>),
        (status = 500, description = "Failed to fetch models"),
    ),
)]
#[get("/api/embedding-inference/models")]
#[tracing::instrument(
    name = "list_embedding-inference_models",
    skip(_user, embedding_inference_config)
)]
pub(crate) async fn list_inference_embedders(
    _user: AuthenticatedUser,
    embedding_inference_config: Data<EmbeddingInferenceConfig>,
) -> impl Responder {
    let url = format!(
        "{}/api/embedders",
        embedding_inference_config.url.trim_end_matches('/')
    );

    match HTTP_CLIENT.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                return ApiError::Internal(format!(
                    "HTTP {}: check inference-api is running",
                    status
                ))
                .error_response();
            }

            match response.json::<Vec<ModelInfo>>().await {
                Ok(mut models) => {
                    // Sort and deduplicate by ID
                    models.sort_by(|a, b| a.id.cmp(&b.id));
                    models.dedup_by(|a, b| a.id == b.id);
                    HttpResponse::Ok().json(models)
                }
                Err(e) => ApiError::Internal(format!("Failed to parse embedder models: {}", e))
                    .error_response(),
            }
        }
        Err(e) => {
            let error_msg = if e.is_timeout() {
                "Request timeout (inference-api may be loading models)".to_string()
            } else if e.is_connect() {
                format!(
                    "Failed to connect to inference-api at {} - is it running?",
                    embedding_inference_config.url
                )
            } else {
                format!("{}", e)
            };
            ApiError::Internal(format!("Failed to fetch embedder models: {}", error_msg))
                .error_response()
        }
    }
}
