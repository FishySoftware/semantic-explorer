use actix_web::{
    HttpResponse, Responder, post,
    web::{Data, Json},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use anyhow::Result;
use qdrant_client::{
    Qdrant,
    qdrant::{
        Condition, FieldCondition, Filter, Match as QdrantMatch, SearchPointsBuilder,
        Value as QdrantValue,
    },
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};

use crate::{
    auth::extract_username,
    storage::postgres::{embedders, transforms},
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
    tracing::debug!(
        "Embeddings keys: {:?}",
        search_request.embeddings.keys().collect::<Vec<_>>()
    );

    if search_request.embeddings.is_empty() {
        return HttpResponse::BadRequest().body("At least one embedder embedding must be provided");
    }

    if search_request.query.trim().is_empty() {
        return HttpResponse::BadRequest().body("Query cannot be empty");
    }

    let user_transforms = match transforms::get_transforms(&postgres_pool, &username).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to fetch transforms: {}", e);
            return HttpResponse::InternalServerError().body("Failed to fetch transforms");
        }
    };

    let dataset_transforms: Vec<_> = user_transforms
        .into_iter()
        .filter(|t| {
            (t.dataset_id == search_request.dataset_id
                || t.source_dataset_id == Some(search_request.dataset_id))
                && t.job_type == "dataset_to_vector_storage"
        })
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

        let collection_name = dataset_transforms.iter().find_map(|t| {
            if let Some(embedder_ids) = &t.embedder_ids
                && embedder_ids.contains(embedder_id)
            {
                return t.get_collection_name(*embedder_id);
            }
            None
        });

        let collection_name = match collection_name {
            Some(name) => name,
            None => {
                results.push(EmbedderSearchResults {
                    embedder_id: *embedder_id,
                    embedder_name: embedder.name,
                    collection_name: String::new(),
                    matches: Vec::new(),
                    error: Some(format!(
                        "No transform found mapping dataset {} to embedder {}",
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
                results.push(EmbedderSearchResults {
                    embedder_id: *embedder_id,
                    embedder_name: embedder.name,
                    collection_name: collection_name.clone(),
                    matches: Vec::new(),
                    error: Some(format!("Search failed: {}", e)),
                });
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

    HttpResponse::Ok().json(SearchResponse {
        results,
        query: search_request.query.clone(),
    })
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct SearchRequest {
    pub query: String,
    pub dataset_id: i32,
    pub embeddings: std::collections::HashMap<i32, Vec<f32>>,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub score_threshold: f32,
    #[serde(default)]
    pub filters: Option<serde_json::Value>,
    #[serde(default)]
    pub search_params: Option<SearchParams>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub(crate) struct SearchParams {
    #[serde(default)]
    pub exact: bool,
    pub hnsw_ef: Option<u64>,
}

fn default_limit() -> u64 {
    10
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct SearchResponse {
    pub results: Vec<EmbedderSearchResults>,
    pub query: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct EmbedderSearchResults {
    pub embedder_id: i32,
    pub embedder_name: String,
    pub collection_name: String,
    pub matches: Vec<SearchMatch>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct SearchMatch {
    pub id: String,
    pub score: f32,
    pub text: String,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
}

async fn search_collection(
    qdrant: &Qdrant,
    collection_name: &str,
    query_vector: &[f32],
    request: &SearchRequest,
) -> Result<Vec<SearchMatch>> {
    let mut search_builder =
        SearchPointsBuilder::new(collection_name, query_vector.to_vec(), request.limit)
            .score_threshold(request.score_threshold)
            .with_payload(true);

    if let Some(filters) = &request.filters
        && let Some(obj) = filters.as_object()
    {
        let mut conditions = Vec::new();

        for (key, value) in obj {
            let condition = if let Some(str_val) = value.as_str() {
                Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(FieldCondition {
                            key: format!("metadata.{}", key),
                            r#match: Some(QdrantMatch {
                                match_value: Some(
                                    qdrant_client::qdrant::r#match::MatchValue::Keyword(
                                        str_val.to_string(),
                                    ),
                                ),
                            }),
                            ..Default::default()
                        }),
                    ),
                }
            } else if let Some(num_val) = value.as_i64() {
                Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(FieldCondition {
                            key: format!("metadata.{}", key),
                            r#match: Some(QdrantMatch {
                                match_value: Some(
                                    qdrant_client::qdrant::r#match::MatchValue::Integer(num_val),
                                ),
                            }),
                            ..Default::default()
                        }),
                    ),
                }
            } else {
                continue;
            };
            conditions.push(condition);
        }

        if !conditions.is_empty() {
            search_builder = search_builder.filter(Filter {
                must: conditions,
                ..Default::default()
            });
        }
    }

    if let Some(params) = &request.search_params
        && let Some(hnsw_ef) = params.hnsw_ef
    {
        search_builder = search_builder.params(
            qdrant_client::qdrant::SearchParamsBuilder::default()
                .hnsw_ef(hnsw_ef)
                .build(),
        );
    }

    let search_result = qdrant.search_points(search_builder).await?;

    let matches = search_result
        .result
        .into_iter()
        .map(|point| {
            let text = point
                .payload
                .get("text")
                .and_then(|v| {
                    if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let mut metadata_map = serde_json::Map::new();
            for (key, value) in &point.payload {
                if key != "text"
                    && let Some(json_val) = qdrant_value_to_json(value)
                {
                    metadata_map.insert(key.clone(), json_val);
                }
            }

            SearchMatch {
                id: point
                    .id
                    .map(|id| match id.point_id_options {
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(u)) => u,
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => {
                            n.to_string()
                        }
                        None => format!("{:?}", id),
                    })
                    .unwrap_or_default(),
                score: point.score,
                text,
                metadata: serde_json::Value::Object(metadata_map),
            }
        })
        .collect();

    Ok(matches)
}

fn qdrant_value_to_json(value: &QdrantValue) -> Option<serde_json::Value> {
    if let Some(kind) = &value.kind {
        match kind {
            qdrant_client::qdrant::value::Kind::NullValue(_) => Some(serde_json::Value::Null),
            qdrant_client::qdrant::value::Kind::BoolValue(b) => Some(serde_json::json!(b)),
            qdrant_client::qdrant::value::Kind::IntegerValue(i) => Some(serde_json::json!(i)),
            qdrant_client::qdrant::value::Kind::DoubleValue(d) => Some(serde_json::json!(d)),
            qdrant_client::qdrant::value::Kind::StringValue(s) => Some(serde_json::json!(s)),
            qdrant_client::qdrant::value::Kind::ListValue(list) => {
                let values: Vec<serde_json::Value> = list
                    .values
                    .iter()
                    .filter_map(qdrant_value_to_json)
                    .collect();
                Some(serde_json::json!(values))
            }
            qdrant_client::qdrant::value::Kind::StructValue(strukt) => {
                let mut map = serde_json::Map::new();
                for (k, v) in &strukt.fields {
                    if let Some(json_val) = qdrant_value_to_json(v) {
                        map.insert(k.clone(), json_val);
                    }
                }
                Some(serde_json::Value::Object(map))
            }
        }
    } else {
        None
    }
}
