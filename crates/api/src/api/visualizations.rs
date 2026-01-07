use actix_web::{
    HttpResponse, Responder, get,
    web::{Data, Path, Query},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use qdrant_client::{Qdrant, qdrant::ScrollPointsBuilder};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tracing::error;

use crate::{auth::extract_username, storage::postgres::visualization_transforms};

#[derive(Deserialize, Debug)]
pub(crate) struct VisualizationPointsQuery {
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: Option<String>,
}

fn default_limit() -> u32 {
    1000
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct VisualizationPoint {
    pub(crate) id: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    pub(crate) cluster_id: Option<i32>,
    pub(crate) topic_label: Option<String>,
    pub(crate) text: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct VisualizationPointsResponse {
    pub(crate) points: Vec<VisualizationPoint>,
    pub(crate) next_offset: Option<String>,
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = VisualizationPointsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Transform not found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Visualizations",
)]
#[get("/api/visualizations/{transform_id}/points")]
#[tracing::instrument(
    name = "get_visualization_points",
    skip(auth, postgres_pool, qdrant_client)
)]
pub(crate) async fn get_visualization_points(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    transform_id: Path<i32>,
    query: Query<VisualizationPointsQuery>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let transform_id = transform_id.into_inner();
    let pool = postgres_pool.into_inner();

    // Get the transform and verify ownership
    let transform =
        match visualization_transforms::get_visualization_transform(&pool, &username, transform_id)
            .await
        {
            Ok(t) => t,
            Err(_) => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "visualization transform not found or access denied"
                }));
            }
        };

    // Get the collection name from reduced_collection_name field
    let collection_name = match &transform.reduced_collection_name {
        Some(name) => name,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "reduced collection not yet created"
            }));
        }
    };

    // Scroll through the Qdrant collection
    let mut scroll_builder = ScrollPointsBuilder::new(collection_name)
        .limit(query.limit)
        .with_payload(true)
        .with_vectors(true);

    if let Some(ref offset) = query.offset {
        scroll_builder = scroll_builder.offset(offset.parse::<u64>().unwrap_or(0));
    }

    let scroll_result = match qdrant_client.scroll(scroll_builder).await {
        Ok(result) => result,
        Err(e) => {
            error!("error scrolling qdrant collection: {e:?}");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("error fetching visualization data: {e:?}")
            }));
        }
    };

    let mut points = Vec::new();

    for point in scroll_result.result {
        // Extract coordinates from vector (2D or 3D)
        if let Some(vectors_output) = point.vectors
            && let Some(vector_options) = vectors_output.vectors_options
        {
            let coords = match vector_options {
                qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(v) => {
                    // Convert VectorOutput to vector_output::Vector enum
                    match v.into_vector() {
                        qdrant_client::qdrant::vector_output::Vector::Dense(dense) => dense.data,
                        _ => continue,
                    }
                }
                _ => continue,
            };

            // Allow both 2D and 3D vectors
            if coords.len() < 2 {
                continue;
            }

            let cluster_id = point
                .payload
                .get("cluster_id")
                .and_then(|v| v.as_integer())
                .map(|i| i as i32);

            let topic_label = point
                .payload
                .get("topic_label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let text = point
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let id_str = point
                .id
                .map(|id| match id.point_id_options {
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(u)) => u,
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => n.to_string(),
                    None => format!("{:?}", id),
                })
                .unwrap_or_default();

            points.push(VisualizationPoint {
                id: id_str,
                x: coords[0],
                y: coords[1],
                z: coords.get(2).copied().unwrap_or(0.0),
                cluster_id,
                topic_label,
                text,
            });
        }
    }

    let next_offset = scroll_result
        .next_page_offset
        .map(|o| match o.point_id_options {
            Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(u)) => u,
            Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => n.to_string(),
            None => format!("{:?}", o),
        });

    HttpResponse::Ok().json(VisualizationPointsResponse {
        points,
        next_offset,
    })
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct VisualizationTopic {
    pub(crate) id: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    pub(crate) cluster_id: i32,
    pub(crate) label: String,
    pub(crate) size: Option<i32>,
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<VisualizationTopic>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Transform not found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Visualizations",
)]
#[get("/api/visualizations/{transform_id}/topics")]
#[tracing::instrument(
    name = "get_visualization_topics",
    skip(auth, postgres_pool, qdrant_client)
)]
pub(crate) async fn get_visualization_topics(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    transform_id: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let transform_id = transform_id.into_inner();
    let pool = postgres_pool.into_inner();

    // Get the transform and verify ownership
    let transform =
        match visualization_transforms::get_visualization_transform(&pool, &username, transform_id)
            .await
        {
            Ok(t) => t,
            Err(_) => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "visualization transform not found or access denied"
                }));
            }
        };

    // Get the topics collection name from topics_collection_name field
    let collection_name = match &transform.topics_collection_name {
        Some(name) => name,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "topics collection not yet created"
            }));
        }
    };

    // Scroll through all topics
    let scroll_builder = ScrollPointsBuilder::new(collection_name)
        .limit(1000)
        .with_payload(true)
        .with_vectors(true);

    let scroll_result = match qdrant_client.scroll(scroll_builder).await {
        Ok(result) => result,
        Err(e) => {
            error!("error scrolling qdrant topics collection: {e:?}");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("error fetching topics: {e:?}")
            }));
        }
    };

    let mut topics = Vec::new();

    for point in scroll_result.result {
        // Extract coordinates from vector
        if let Some(vectors_output) = point.vectors
            && let Some(vector_options) = vectors_output.vectors_options
        {
            let coords = match vector_options {
                qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(v) => {
                    // Convert VectorOutput to vector_output::Vector enum
                    match v.into_vector() {
                        qdrant_client::qdrant::vector_output::Vector::Dense(dense) => dense.data,
                        _ => continue,
                    }
                }
                _ => continue,
            };

            if coords.len() < 2 {
                continue;
            }

            let cluster_id = point
                .payload
                .get("cluster_id")
                .and_then(|v| v.as_integer())
                .unwrap_or(-1) as i32;

            let label = point
                .payload
                .get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let size = point
                .payload
                .get("size")
                .and_then(|v| v.as_integer())
                .map(|i| i as i32);

            let id_str = point
                .id
                .map(|id| match id.point_id_options {
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(u)) => u,
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => n.to_string(),
                    None => format!("{:?}", id),
                })
                .unwrap_or_default();

            topics.push(VisualizationTopic {
                id: id_str,
                x: coords[0],
                y: coords[1],
                z: coords.get(2).copied().unwrap_or(0.0),
                cluster_id,
                label,
                size,
            });
        }
    }

    HttpResponse::Ok().json(topics)
}
