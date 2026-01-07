use actix_web::{
    HttpResponse, Responder, post,
    web::{Data, Json},
};
use actix_web_openidconnect::openid_middleware::Authenticated;

use qdrant_client::Qdrant;
use sqlx::{Pool, Postgres};

use crate::{
    auth::extract_username,
    search::{
        aggregate_by_source,
        models::{EmbedderSearchResults, SearchMode, SearchRequest, SearchResponse},
        search_collection,
    },
    storage::postgres::{embedded_datasets, embedders},
};

#[utoipa::path(
    request_body = SearchRequest,
    responses(
        (status = 200, description = "OK", body = SearchResponse),
        (status = 400, description = "Bad Request"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Search",
)]
#[post("/api/search")]
#[tracing::instrument(
    name = "search",
    skip(auth, qdrant_client, postgres_pool, search_request)
)]
pub(crate) async fn search(
    auth: Authenticated,
    qdrant_client: Data<Qdrant>,
    postgres_pool: Data<Pool<Postgres>>,
    Json(search_request): Json<SearchRequest>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    tracing::info!(
        "Received search request, embeddings count: {}, query: '{}'",
        search_request.embeddings.len(),
        search_request.query
    );

    if search_request.embeddings.is_empty() {
        return HttpResponse::BadRequest().body("At least one embedder embedding must be provided");
    }

    if search_request.query.trim().is_empty() {
        return HttpResponse::BadRequest().body("Query cannot be empty");
    }

    let embedded_datasets_list = match embedded_datasets::get_embedded_datasets_for_dataset(
        &postgres_pool,
        &username,
        search_request.dataset_id,
    )
    .await
    {
        Ok(eds) => eds,
        Err(e) => {
            tracing::error!("Failed to fetch embedded datasets: {}", e);
            return HttpResponse::InternalServerError().body("Failed to fetch embedded datasets");
        }
    };

    // Build a map from embedder_id to collection_name
    let embedder_collection_map: std::collections::HashMap<i32, String> = embedded_datasets_list
        .into_iter()
        .map(|ed| (ed.embedder_id, ed.collection_name))
        .collect();

    let mut results = Vec::new();

    for (embedder_id, query_vector) in &search_request.embeddings {
        // Get embedder details
        let embedder = match embedders::get_embedder(&postgres_pool, &username, *embedder_id).await
        {
            Ok(e) => e,
            Err(e) => {
                results.push(EmbedderSearchResults {
                    embedder_id: *embedder_id,
                    embedder_name: format!("Embedder {}", embedder_id),
                    collection_name: String::new(),
                    matches: Vec::new(),
                    error: Some(format!("Failed to fetch embedder: {}", e)),
                });
                continue;
            }
        };

        let collection_name = match embedder_collection_map.get(embedder_id) {
            Some(name) => name.clone(),
            None => {
                results.push(EmbedderSearchResults {
                    embedder_id: *embedder_id,
                    embedder_name: embedder.name,
                    collection_name: String::new(),
                    matches: Vec::new(),
                    error: Some(format!(
                        "No embedded dataset found for dataset {} with embedder {}",
                        search_request.dataset_id, embedder_id
                    )),
                });
                continue;
            }
        };

        let matches = match search_collection(
            &qdrant_client,
            &collection_name,
            query_vector,
            &search_request,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("doesn't exist")
                    || error_msg.contains("not found")
                    || error_msg.contains("No collection")
                {
                    tracing::warn!(
                        "Collection '{}' does not exist for embedder {}, dataset might not be processed yet",
                        collection_name,
                        embedder_id
                    );
                    results.push(EmbedderSearchResults {
                        embedder_id: *embedder_id,
                        embedder_name: embedder.name,
                        collection_name: collection_name.clone(),
                        matches: Vec::new(),
                        error: Some(
                            "This dataset has not been embedded yet with this embedder. Please wait for the embedding process to complete.".to_string()
                        ),
                    });
                } else {
                    results.push(EmbedderSearchResults {
                        embedder_id: *embedder_id,
                        embedder_name: embedder.name,
                        collection_name: collection_name.clone(),
                        matches: Vec::new(),
                        error: Some(format!("Search failed: {}", e)),
                    });
                }
                continue;
            }
        };

        results.push(EmbedderSearchResults {
            embedder_id: *embedder_id,
            embedder_name: embedder.name,
            collection_name,
            matches,
            error: None,
        });
    }

    // Aggregate by source if requested
    let aggregated_sources = if matches!(search_request.search_mode, SearchMode::Sources) {
        Some(aggregate_by_source(&results))
    } else {
        None
    };

    HttpResponse::Ok().json(SearchResponse {
        results,
        query: search_request.query.clone(),
        search_mode: search_request.search_mode,
        aggregated_sources,
    })
}
