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
        aggregate_matches_to_documents,
        models::{EmbeddedDatasetSearchResults, SearchMode, SearchRequest, SearchResponse},
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


    if search_request.embedded_dataset_ids.is_empty() {
        return HttpResponse::BadRequest()
            .body("At least one embedded dataset must be selected");
    }

    if search_request.query.trim().is_empty() {
        return HttpResponse::BadRequest().body("Query cannot be empty");
    }

    let mut results = Vec::new();

    for embedded_dataset_id in &search_request.embedded_dataset_ids {
        let ed_details = match embedded_datasets::get_embedded_dataset_with_details(
            &postgres_pool,
            &username,
            *embedded_dataset_id,
        )
        .await
        {
            Ok(ed) => ed,
            Err(e) => {
                tracing::error!(
                    "Failed to fetch embedded dataset {}: {}",
                    embedded_dataset_id,
                    e
                );
                results.push(EmbeddedDatasetSearchResults {
                    embedded_dataset_id: *embedded_dataset_id,
                    embedded_dataset_title: format!("Embedded Dataset {}", embedded_dataset_id),
                    source_dataset_id: 0,
                    source_dataset_title: String::new(),
                    embedder_id: 0,
                    embedder_name: String::new(),
                    collection_name: String::new(),
                    matches: Vec::new(),
                    documents: None,
                    error: Some(format!("Failed to fetch embedded dataset: {}", e)),
                });
                continue;
            }
        };

        let embedder = match embedders::get_embedder(&postgres_pool, &username, ed_details.embedder_id)
            .await
        {
            Ok(e) => e,
            Err(e) => {
                results.push(EmbeddedDatasetSearchResults {
                    embedded_dataset_id: *embedded_dataset_id,
                    embedded_dataset_title: ed_details.title,
                    source_dataset_id: ed_details.source_dataset_id,
                    source_dataset_title: ed_details.source_dataset_title,
                    embedder_id: ed_details.embedder_id,
                    embedder_name: ed_details.embedder_name,
                    collection_name: ed_details.collection_name,
                    matches: Vec::new(),
                    documents: None,
                    error: Some(format!("Failed to fetch embedder: {}", e)),
                });
                continue;
            }
        };


        let query_vector = match crate::embedding::generate_embedding(
            &embedder.provider,
            &embedder.base_url,
            embedder.api_key.as_deref(),
            &embedder.config,
            &search_request.query,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(
                    "Failed to generate embedding for embedded dataset {}: {}",
                    embedded_dataset_id,
                    e
                );
                results.push(EmbeddedDatasetSearchResults {
                    embedded_dataset_id: *embedded_dataset_id,
                    embedded_dataset_title: ed_details.title,
                    source_dataset_id: ed_details.source_dataset_id,
                    source_dataset_title: ed_details.source_dataset_title,
                    embedder_id: ed_details.embedder_id,
                    embedder_name: ed_details.embedder_name,
                    collection_name: ed_details.collection_name.clone(),
                    matches: Vec::new(),
                    documents: None,
                    error: Some(format!("Failed to generate embedding: {}", e)),
                });
                continue;
            }
        };

        let matches = match search_collection(
            &qdrant_client,
            &ed_details.collection_name,
            &query_vector,
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
                        "Collection '{}' does not exist, embedded dataset might not be processed yet",
                        ed_details.collection_name
                    );
                    results.push(EmbeddedDatasetSearchResults {
                        embedded_dataset_id: *embedded_dataset_id,
                        embedded_dataset_title: ed_details.title,
                        source_dataset_id: ed_details.source_dataset_id,
                        source_dataset_title: ed_details.source_dataset_title,
                        embedder_id: ed_details.embedder_id,
                        embedder_name: ed_details.embedder_name,
                        collection_name: ed_details.collection_name,
                        matches: Vec::new(),
                        documents: None,
                        error: Some(
                            "This embedded dataset has not been processed yet. Please wait for the embedding process to complete.".to_string()
                        ),
                    });
                } else {
                    results.push(EmbeddedDatasetSearchResults {
                        embedded_dataset_id: *embedded_dataset_id,
                        embedded_dataset_title: ed_details.title,
                        source_dataset_id: ed_details.source_dataset_id,
                        source_dataset_title: ed_details.source_dataset_title,
                        embedder_id: ed_details.embedder_id,
                        embedder_name: ed_details.embedder_name,
                        collection_name: ed_details.collection_name,
                        matches: Vec::new(),
                        documents: None,
                        error: Some(format!("Search failed: {}", e)),
                    });
                }
                continue;
            }
        };

        let documents = if matches!(search_request.search_mode, SearchMode::Documents) {
            Some(aggregate_matches_to_documents(&matches))
        } else {
            None
        };

        results.push(EmbeddedDatasetSearchResults {
            embedded_dataset_id: *embedded_dataset_id,
            embedded_dataset_title: ed_details.title,
            source_dataset_id: ed_details.source_dataset_id,
            source_dataset_title: ed_details.source_dataset_title,
            embedder_id: ed_details.embedder_id,
            embedder_name: ed_details.embedder_name,
            collection_name: ed_details.collection_name,
            matches,
            documents,
            error: None,
        });
    }

    HttpResponse::Ok().json(SearchResponse {
        results,
        query: search_request.query.clone(),
        search_mode: search_request.search_mode,
    })
}
