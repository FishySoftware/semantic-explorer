//! Reranking API endpoints.

use actix_web::{HttpResponse, Responder, ResponseError, get, post, web};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::ToSchema;

use crate::config::ModelConfig;
use crate::models::get_reranker_models;
use crate::reranker;

/// Document to be reranked
#[derive(Debug, Deserialize, ToSchema)]
pub struct Document {
    /// Document text content
    pub text: String,
    /// Optional document ID for tracking
    #[serde(default)]
    pub id: Option<String>,
}

/// Request body for reranking
#[derive(Debug, Deserialize, ToSchema)]
pub struct RerankRequest {
    /// Query to rank documents against
    pub query: String,
    /// Documents to rerank
    pub documents: Vec<Document>,
    /// Model to use (required)
    pub model: String,
    /// Number of top results to return (defaults to all)
    #[serde(default)]
    pub top_k: Option<usize>,
    /// Whether to return the relevance scores
    #[serde(default = "default_return_scores")]
    pub return_scores: bool,
}

fn default_return_scores() -> bool {
    true
}

/// A reranked document result
#[derive(Debug, Serialize, ToSchema)]
pub struct RerankResult {
    /// Original index in the input documents
    pub index: usize,
    /// Document ID if provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Document text
    pub text: String,
    /// Relevance score (higher is more relevant)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

/// Response for reranking requests
#[derive(Debug, Serialize, ToSchema)]
pub struct RerankResponse {
    /// Reranked documents in order of relevance
    pub results: Vec<RerankResult>,
    /// Model used
    pub model: String,
}

/// List available reranker models only
#[utoipa::path(
    get,
    path = "/api/rerankers",
    responses(
        (status = 200, description = "List of available reranker models", body = Vec<crate::models::ModelInfo>),
        (status = 500, description = "Internal server error")
    ),
    tag = "models"
)]
#[get("/api/rerankers")]
#[instrument(skip(config))]
pub async fn list_rerankers(config: web::Data<ModelConfig>) -> impl Responder {
    let rerankers = get_reranker_models(&config);
    HttpResponse::Ok().json(rerankers)
}

/// Rerank documents by relevance to a query
#[utoipa::path(
    post,
    path = "/api/rerank",
    request_body = RerankRequest,
    responses(
        (status = 200, description = "Documents reranked successfully", body = RerankResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    tag = "reranking"
)]
#[post("/api/rerank")]
#[instrument(skip(config, body), fields(model, doc_count = body.documents.len()))]
pub async fn rerank(
    config: web::Data<ModelConfig>,
    body: web::Json<RerankRequest>,
) -> impl Responder {
    if body.documents.is_empty() {
        return HttpResponse::Ok().json(RerankResponse {
            results: vec![],
            model: body.model.clone(),
        });
    }

    let model_id = &body.model;

    tracing::Span::current().record("model", model_id);

    // Prepare documents for reranking - need Vec<&str> for the API
    let texts: Vec<&str> = body.documents.iter().map(|d| d.text.as_str()).collect();
    let document_count = body.documents.len() as u64;

    let start = std::time::Instant::now();
    let result = reranker::rerank_documents(model_id, &config, &body.query, &texts, body.top_k);
    let duration = start.elapsed().as_secs_f64();

    match result {
        Ok(rerank_results) => {
            semantic_explorer_core::observability::record_rerank_request(
                model_id,
                document_count,
                duration,
                true,
            );

            // Build response with original document info
            let results: Vec<RerankResult> = rerank_results
                .into_iter()
                .map(|r| {
                    let original_doc = &body.documents[r.index];
                    RerankResult {
                        index: r.index,
                        id: original_doc.id.clone(),
                        text: r.document.unwrap_or_default(),
                        score: if body.return_scores {
                            Some(r.score)
                        } else {
                            None
                        },
                    }
                })
                .collect();

            HttpResponse::Ok().json(RerankResponse {
                results,
                model: model_id.clone(),
            })
        }
        Err(e) => {
            semantic_explorer_core::observability::record_rerank_request(
                model_id,
                document_count,
                duration,
                false,
            );
            e.error_response()
        }
    }
}
