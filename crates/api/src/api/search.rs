use std::collections::{HashMap, HashSet};

use actix_web::{
    HttpResponse, Responder, ResponseError, post,
    web::{Data, Json},
};
use futures_util::future;

use qdrant_client::Qdrant;
use sqlx::{Pool, Postgres};

use crate::{
    auth::AuthenticatedUser,
    errors::ApiError,
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
    skip(user, qdrant_client, postgres_pool, search_request)
)]
pub(crate) async fn search(
    user: AuthenticatedUser,
    qdrant_client: Data<Qdrant>,
    postgres_pool: Data<Pool<Postgres>>,
    Json(search_request): Json<SearchRequest>,
) -> impl Responder {
    let start_time = std::time::Instant::now();

    if search_request.embedded_dataset_ids.is_empty() {
        return ApiError::BadRequest("At least one embedded dataset must be selected".to_string())
            .error_response();
    }

    if search_request.query.trim().is_empty() {
        return ApiError::BadRequest("Query cannot be empty".to_string()).error_response();
    }

    // Batch fetch all embedded datasets and embedders upfront to avoid N+1 queries
    let embedded_datasets_map = match embedded_datasets::get_embedded_datasets_with_details_batch(
        &postgres_pool,
        &user,
        &search_request.embedded_dataset_ids,
    )
    .await
    {
        Ok(eds) => {
            // Convert to HashMap for fast lookup
            eds.into_iter()
                .map(|ed| (ed.embedded_dataset_id, ed))
                .collect::<HashMap<_, _>>()
        }
        Err(e) => {
            tracing::error!("Failed to fetch embedded datasets in batch: {}", e);
            return ApiError::Internal(format!("Failed to fetch embedded datasets: {}", e))
                .error_response();
        }
    };

    // Extract unique embedder IDs
    let embedder_ids: Vec<i32> = embedded_datasets_map
        .values()
        .map(|ed| ed.embedder_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let embedders_map =
        match embedders::get_embedders_batch(&postgres_pool, &user, &embedder_ids).await {
            Ok(embs) => {
                // Convert to HashMap for fast lookup
                embs.into_iter()
                    .map(|emb| (emb.embedder_id, emb))
                    .collect::<HashMap<_, _>>()
            }
            Err(e) => {
                tracing::error!("Failed to fetch embedders in batch: {}", e);
                return ApiError::Internal(format!("Failed to fetch embedders: {}", e))
                    .error_response();
            }
        };

    // Process searches in parallel using futures::future::join_all
    let search_tasks: Vec<_> = search_request
        .embedded_dataset_ids
        .iter()
        .map(|embedded_dataset_id| {
            let qdrant_client = qdrant_client.clone();
            let search_request = search_request.clone();
            let ed_details = embedded_datasets_map.get(embedded_dataset_id).cloned();
            let embedder = ed_details
                .as_ref()
                .and_then(|ed| embedders_map.get(&ed.embedder_id).cloned());

            async move {
                let embedded_dataset_id = *embedded_dataset_id;

                // Check if we have the embedded dataset
                let ed_details = match ed_details {
                    Some(ed) => ed,
                    None => {
                        return EmbeddedDatasetSearchResults {
                            embedded_dataset_id,
                            embedded_dataset_title: format!("Embedded Dataset {}", embedded_dataset_id),
                            source_dataset_id: 0,
                            source_dataset_title: String::new(),
                            embedder_id: 0,
                            embedder_name: String::new(),
                            collection_name: String::new(),
                            matches: Vec::new(),
                            documents: None,
                            error: Some("Embedded dataset not found or not accessible".to_string()),
                        };
                    }
                };

                // Check if we have the embedder
                let embedder = match embedder {
                    Some(emb) => emb,
                    None => {
                        return EmbeddedDatasetSearchResults {
                            embedded_dataset_id,
                            embedded_dataset_title: ed_details.title,
                            source_dataset_id: ed_details.source_dataset_id,
                            source_dataset_title: ed_details.source_dataset_title,
                            embedder_id: ed_details.embedder_id,
                            embedder_name: ed_details.embedder_name,
                            collection_name: ed_details.collection_name,
                            matches: Vec::new(),
                            documents: None,
                            error: Some("Embedder not found or not accessible".to_string()),
                        };
                    }
                };

                // Generate embedding for the query
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
                        return EmbeddedDatasetSearchResults {
                            embedded_dataset_id,
                            embedded_dataset_title: ed_details.title,
                            source_dataset_id: ed_details.source_dataset_id,
                            source_dataset_title: ed_details.source_dataset_title,
                            embedder_id: ed_details.embedder_id,
                            embedder_name: ed_details.embedder_name,
                            collection_name: ed_details.collection_name.clone(),
                            matches: Vec::new(),
                            documents: None,
                            error: Some(format!("Failed to generate embedding: {}", e)),
                        };
                    }
                };

                // Perform the search
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
                            return EmbeddedDatasetSearchResults {
                                embedded_dataset_id,
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
                            };
                        } else {
                            return EmbeddedDatasetSearchResults {
                                embedded_dataset_id,
                                embedded_dataset_title: ed_details.title,
                                source_dataset_id: ed_details.source_dataset_id,
                                source_dataset_title: ed_details.source_dataset_title,
                                embedder_id: ed_details.embedder_id,
                                embedder_name: ed_details.embedder_name,
                                collection_name: ed_details.collection_name,
                                matches: Vec::new(),
                                documents: None,
                                error: Some(format!("Search failed: {}", e)),
                            };
                        }
                    }
                };

                let documents = if matches!(search_request.search_mode, SearchMode::Documents) {
                    Some(aggregate_matches_to_documents(&matches))
                } else {
                    None
                };

                EmbeddedDatasetSearchResults {
                    embedded_dataset_id,
                    embedded_dataset_title: ed_details.title,
                    source_dataset_id: ed_details.source_dataset_id,
                    source_dataset_title: ed_details.source_dataset_title,
                    embedder_id: ed_details.embedder_id,
                    embedder_name: ed_details.embedder_name,
                    collection_name: ed_details.collection_name,
                    matches,
                    documents,
                    error: None,
                }
            }
        })
        .collect();

    // Execute all searches in parallel
    let results = future::join_all(search_tasks).await;

    let duration = start_time.elapsed().as_secs_f64();
    let total_results: usize = results.iter().map(|r| r.matches.len()).sum();
    let embedded_datasets_count = search_request.embedded_dataset_ids.len();

    // Record search metrics
    semantic_explorer_core::observability::record_search_request(
        duration,
        0.0, // embedder_duration_secs - would need to track separately
        0.0, // qdrant_duration_secs - would need to track separately
        total_results,
        embedded_datasets_count,
        "success",
    );

    HttpResponse::Ok().json(SearchResponse {
        results,
        query: search_request.query.clone(),
        search_mode: search_request.search_mode,
    })
}
