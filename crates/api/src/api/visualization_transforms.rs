use crate::auth::extract_username;
use crate::storage::postgres::visualization_transforms;
use crate::transforms::visualization::{
    CreateVisualizationTransform, UpdateVisualizationTransform, VisualizationTransform,
    VisualizationTransformStats,
};

use actix_web::web::{Data, Json, Path};
use actix_web::{HttpResponse, Responder, delete, get, patch, post};
use actix_web_openidconnect::openid_middleware::Authenticated;
use sqlx::{Pool, Postgres};
use tracing::error;

#[utoipa::path(
    get,
    path = "/api/visualization-transforms",
    tag = "Visualization Transforms",
    responses(
        (status = 200, description = "List of visualization transforms", body = Vec<VisualizationTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms")]
#[tracing::instrument(name = "get_visualization_transforms", skip(auth, postgres_pool))]
pub async fn get_visualization_transforms(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match visualization_transforms::get_visualization_transforms(&postgres_pool, &username).await {
        Ok(transforms) => HttpResponse::Ok().json(transforms),
        Err(e) => {
            error!("Failed to fetch visualization transforms: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transforms: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID")
    ),
    responses(
        (status = 200, description = "Visualization transform details", body = VisualizationTransform),
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}")]
#[tracing::instrument(name = "get_visualization_transform", skip(auth, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualization_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match visualization_transforms::get_visualization_transform(
        &postgres_pool,
        &username,
        path.into_inner(),
    )
    .await
    {
        Ok(transform) => HttpResponse::Ok().json(transform),
        Err(e) => {
            error!("Visualization transform not found: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Visualization transform not found: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/visualization-transforms",
    tag = "Visualization Transforms",
    request_body = CreateVisualizationTransform,
    responses(
        (status = 201, description = "Visualization transform created", body = VisualizationTransform),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/visualization-transforms")]
#[tracing::instrument(name = "create_visualization_transform", skip(auth, postgres_pool, body), fields(title = %body.title))]
pub async fn create_visualization_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    body: Json<CreateVisualizationTransform>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let visualization_config = serde_json::to_value(&body.visualization_config).unwrap();

    match visualization_transforms::create_visualization_transform(
        &postgres_pool,
        &body.title,
        body.embedded_dataset_id,
        &username,
        &visualization_config,
    )
    .await
    {
        Ok(transform) => HttpResponse::Created().json(transform),
        Err(e) => {
            error!("Failed to create visualization transform: {}", e);
            HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Failed to create visualization transform: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/visualization-transforms/{id}",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID")
    ),
    request_body = UpdateVisualizationTransform,
    responses(
        (status = 200, description = "Visualization transform updated", body = VisualizationTransform),
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[patch("/api/visualization-transforms/{id}")]
#[tracing::instrument(name = "update_visualization_transform", skip(auth, postgres_pool, body), fields(visualization_transform_id = %path.as_ref()))]
pub async fn update_visualization_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateVisualizationTransform>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let visualization_config = body
        .visualization_config
        .as_ref()
        .map(|config| serde_json::to_value(config).unwrap());

    match visualization_transforms::update_visualization_transform(
        &postgres_pool,
        &username,
        path.into_inner(),
        body.title.as_deref(),
        body.is_enabled,
        visualization_config.as_ref(),
    )
    .await
    {
        Ok(transform) => HttpResponse::Ok().json(transform),
        Err(e) => {
            error!("Failed to update visualization transform: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Failed to update visualization transform: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/visualization-transforms/{id}",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID")
    ),
    responses(
        (status = 204, description = "Visualization transform deleted"),
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[delete("/api/visualization-transforms/{id}")]
#[tracing::instrument(name = "delete_visualization_transform", skip(auth, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn delete_visualization_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match visualization_transforms::delete_visualization_transform(
        &postgres_pool,
        &username,
        path.into_inner(),
    )
    .await
    {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(e) => {
            error!("Failed to delete visualization transform: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Failed to delete visualization transform: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/visualization-transforms/{id}/trigger",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID")
    ),
    responses(
        (status = 200, description = "Visualization transform triggered"),
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/visualization-transforms/{id}/trigger")]
#[tracing::instrument(name = "trigger_visualization_transform", skip(auth, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn trigger_visualization_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let visualization_transform_id = path.into_inner();

    match visualization_transforms::get_visualization_transform(
        &postgres_pool,
        &username,
        visualization_transform_id,
    )
    .await
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "message": "Visualization transform triggered",
            "visualization_transform_id": visualization_transform_id
        })),
        Err(e) => {
            error!("Visualization transform not found: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Visualization transform not found: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/stats",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID")
    ),
    responses(
        (status = 200, description = "Visualization transform statistics", body = VisualizationTransformStats),
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/stats")]
#[tracing::instrument(name = "get_visualization_transform_stats", skip(auth, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualization_transform_stats(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let visualization_transform_id = path.into_inner();

    match visualization_transforms::get_visualization_transform(
        &postgres_pool,
        &username,
        visualization_transform_id,
    )
    .await
    {
        Ok(_) => {
            match visualization_transforms::get_visualization_transform_stats(
                &postgres_pool,
                visualization_transform_id,
            )
            .await
            {
                Ok(stats) => HttpResponse::Ok().json(stats),
                Err(e) => {
                    error!("Failed to get stats: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to get stats: {}", e)
                    }))
                }
            }
        }
        Err(e) => {
            error!("Visualization transform not found: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Visualization transform not found: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/points",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID")
    ),
    responses(
        (status = 200, description = "3D visualization points", body = Vec<serde_json::Value>),  // TODO: Replace with VisualizationPoint type
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/points")]
#[tracing::instrument(name = "get_visualization_points", skip(auth, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualization_points(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let visualization_transform_id = path.into_inner();

    match visualization_transforms::get_visualization_transform(
        &postgres_pool,
        &username,
        visualization_transform_id,
    )
    .await
    {
        Ok(_transform) => {
            // TODO: Implement Qdrant query to fetch 3D points from reduced_collection_name
            // For now, return placeholder
            HttpResponse::Ok().json(Vec::<serde_json::Value>::new())
        }
        Err(e) => {
            error!("Visualization transform not found: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Visualization transform not found: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/topics",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID")
    ),
    responses(
        (status = 200, description = "Topic cluster information", body = Vec<serde_json::Value>),  // TODO: Replace with VisualizationTopic type
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/topics")]
#[tracing::instrument(name = "get_visualization_topics", skip(auth, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualization_topics(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let visualization_transform_id = path.into_inner();

    match visualization_transforms::get_visualization_transform(
        &postgres_pool,
        &username,
        visualization_transform_id,
    )
    .await
    {
        Ok(_transform) => {
            // TODO: Implement Qdrant query to fetch topics from topics_collection_name
            // For now, return placeholder
            HttpResponse::Ok().json(Vec::<serde_json::Value>::new())
        }
        Err(e) => {
            error!("Visualization transform not found: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("Visualization transform not found: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets/{embedded_dataset_id}/visualizations",
    tag = "Visualization Transforms",
    params(
        ("embedded_dataset_id" = i32, Path, description = "Embedded Dataset ID")
    ),
    responses(
        (status = 200, description = "Visualizations for embedded dataset", body = Vec<VisualizationTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{embedded_dataset_id}/visualizations")]
#[tracing::instrument(name = "get_visualizations_for_embedded_dataset", skip(auth, postgres_pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_visualizations_for_embedded_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match visualization_transforms::get_visualization_transforms_for_embedded_dataset(
        &postgres_pool,
        &username,
        path.into_inner(),
    )
    .await
    {
        Ok(transforms) => HttpResponse::Ok().json(transforms),
        Err(e) => {
            error!("Failed to fetch visualization transforms: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transforms: {}", e)
            }))
        }
    }
}
