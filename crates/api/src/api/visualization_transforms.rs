use crate::audit::{ResourceType, events};
use crate::auth::AuthenticatedUser;
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::{embedded_datasets, llms, visualization_transforms};
use crate::transforms::visualization::models::{
    CreateVisualizationTransform, UpdateVisualizationTransform, Visualization,
    VisualizationTransform, VisualizationTransformStats,
};
use crate::transforms::visualization::scanner::trigger_visualization_transform_scan;
use semantic_explorer_core::encryption::EncryptionService;
use semantic_explorer_core::models::PaginatedResponse;
use semantic_explorer_core::validation;

use actix_web::web::{Data, Json, Path, Query};
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, patch, post};
use async_nats::Client as NatsClient;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tracing::{error, info};

#[derive(Deserialize, Debug)]
pub struct SortParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_direction")]
    pub sort_direction: String,
    pub search: Option<String>,
}

fn default_limit() -> i64 {
    10
}

fn default_sort_by() -> String {
    "created_at".to_string()
}

fn default_sort_direction() -> String {
    "desc".to_string()
}

#[derive(Deserialize, Debug)]
pub struct PaginationParams {
    #[serde(default = "default_pagination_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_pagination_limit() -> i64 {
    50
}

#[utoipa::path(
	get,
	path = "/api/visualization-transforms",
    tag = "Visualization Transforms",
    params(
        ("limit" = i64, Query, description = "Number of results per page", example = 10),
        ("offset" = i64, Query, description = "Number of results to skip", example = 0),
        ("sort_by" = String, Query, description = "Field to sort by: title, is_enabled, last_run_status, created_at, updated_at", example = "created_at"),
        ("sort_direction" = String, Query, description = "Sort direction: asc or desc", example = "desc"),
        ("search" = Option<String>, Query, description = "Search term to filter transforms by title"),
    ),
    responses(
        (status = 200, description = "Paginated list of visualization transforms", body = PaginatedResponse<VisualizationTransform>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms")]
#[tracing::instrument(
    name = "get_visualization_transforms",
    skip(user, postgres_pool, params)
)]
pub async fn get_visualization_transforms(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    params: Query<SortParams>,
) -> impl Responder {
    match visualization_transforms::get_visualization_transforms_paginated(
        &postgres_pool,
        &user,
        params.limit,
        params.offset,
        &params.sort_by,
        &params.sort_direction,
        params.search.as_deref(),
    )
    .await
    {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            error!("Failed to fetch visualization transforms: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch visualization transforms"
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
#[tracing::instrument(name = "get_visualization_transform", skip(user, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualization_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != *user {
                events::unauthorized_access(
                    &user,
                    ResourceType::Visualization,
                    &id.to_string(),
                    "user does not own this visualization transform",
                );
                return not_found("Visualization transform not found".to_string());
            }
            events::resource_read(&user, ResourceType::Visualization, &id.to_string());
            HttpResponse::Ok().json(transform)
        }
        Ok(None) => not_found("Visualization transform not found".to_string()),
        Err(e) => {
            error!("Failed to fetch visualization transform: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transform: {}", e)
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
        (status = 201, description = "Visualization transform created and triggered", body = VisualizationTransform),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[post("/api/visualization-transforms")]
#[tracing::instrument(name = "create_visualization_transform", skip(user, postgres_pool, nats_client, body, req, encryption), fields(title = %body.title))]
pub async fn create_visualization_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    encryption: Data<EncryptionService>,
    body: Json<CreateVisualizationTransform>,
) -> impl Responder {
    // Validate input
    if let Err(e) = validation::validate_title(&body.title) {
        return bad_request(e);
    }

    // Verify embedded dataset exists and belongs to user
    match embedded_datasets::get_embedded_dataset(&postgres_pool, &user, body.embedded_dataset_id)
        .await
    {
        Ok(dataset) => {
            if dataset.owner != *user {
                return bad_request("Embedded dataset not found or access denied");
            }
        }
        Err(e) => {
            error!("Failed to verify embedded dataset: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to verify embedded dataset"
            }));
        }
    }

    // If LLM ID provided, verify it exists and belongs to user
    if let Some(llm_id) = body.llm_id {
        match llms::get_llm(&postgres_pool, &user, llm_id, &encryption).await {
            Ok(llm) => {
                if llm.owner != *user {
                    return bad_request("LLM not found or access denied");
                }
            }
            Err(e) => {
                error!("Failed to verify LLM: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to verify LLM"
                }));
            }
        }
    }

    // Build visualization config
    let visualization_config = serde_json::json!({
        "n_neighbors": body.n_neighbors,
        "min_dist": body.min_dist,
        "metric": body.metric,
        "min_cluster_size": body.min_cluster_size,
        "min_samples": body.min_samples,
        "topic_naming_llm_id": body.llm_id,
        "llm_batch_size": body.llm_batch_size,
        "samples_per_cluster": body.samples_per_cluster,
        // Datamapplot visualization parameters
        "min_fontsize": body.min_fontsize,
        "max_fontsize": body.max_fontsize,
        "font_family": body.font_family,
        "darkmode": body.darkmode,
        "noise_color": body.noise_color,
        "label_wrap_width": body.label_wrap_width,
        "use_medoids": body.use_medoids,
        "cluster_boundary_polygons": body.cluster_boundary_polygons,
        "polygon_alpha": body.polygon_alpha,
    });

    match visualization_transforms::create_visualization_transform(
        &postgres_pool,
        &body.title,
        body.embedded_dataset_id,
        &user,
        &visualization_config,
    )
    .await
    {
        Ok(transform) => {
            // Auto-trigger scan on creation
            let transform_id = transform.visualization_transform_id;
            info!(
                "Created visualization transform {}, triggering initial scan",
                transform_id
            );

            if let Err(e) = trigger_visualization_transform_scan(
                &postgres_pool,
                &nats_client,
                transform_id,
                &user,
                &encryption,
            )
            .await
            {
                error!(
                    "Failed to trigger visualization transform scan for newly created transform {}: {}",
                    transform_id, e
                );
                // Don't fail the creation, just log the error
            }

            events::resource_created_with_request(
                &req,
                &user,
                ResourceType::Visualization,
                &transform_id.to_string(),
            );
            HttpResponse::Created().json(transform)
        }
        Err(e) => {
            error!("Failed to create visualization transform: {}", e);
            bad_request(format!("Failed to create visualization transform: {}", e))
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
#[tracing::instrument(name = "update_visualization_transform", skip(user, postgres_pool, body), fields(visualization_transform_id = %path.as_ref()))]
pub async fn update_visualization_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    body: Json<UpdateVisualizationTransform>,
) -> impl Responder {
    // Validate input if title is provided
    if let Some(ref title) = body.title
        && let Err(e) = validation::validate_title(title)
    {
        return bad_request(e);
    }

    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != *user {
                return not_found("Visualization transform not found".to_string());
            }
        }
        Ok(None) => return not_found("Visualization transform not found".to_string()),
        Err(e) => {
            error!("Failed to fetch visualization transform: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transform: {}", e)
            }));
        }
    }

    match visualization_transforms::update_visualization_transform(
        &postgres_pool,
        id,
        &user,
        body.title.as_deref(),
        body.is_enabled,
        body.visualization_config.as_ref(),
    )
    .await
    {
        Ok(transform) => {
            events::resource_updated(&user, ResourceType::Visualization, &id.to_string());
            HttpResponse::Ok().json(transform)
        }
        Err(e) => {
            error!("Failed to update visualization transform: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
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
#[tracing::instrument(name = "delete_visualization_transform", skip(user, postgres_pool, req), fields(visualization_transform_id = %path.as_ref()))]
pub async fn delete_visualization_transform(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != *user {
                return not_found("Visualization transform not found".to_string());
            }
        }
        Ok(None) => return not_found("Visualization transform not found".to_string()),
        Err(e) => {
            error!("Failed to fetch visualization transform: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transform: {}", e)
            }));
        }
    }

    match visualization_transforms::delete_visualization_transform(&postgres_pool, id, &user).await
    {
        Ok(()) => {
            events::resource_deleted_with_request(
                &req,
                &user,
                ResourceType::Visualization,
                &id.to_string(),
            );
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            error!("Failed to delete visualization transform: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
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
#[tracing::instrument(name = "trigger_visualization_transform", skip(user, postgres_pool, nats_client, encryption), fields(visualization_transform_id = %path.as_ref()))]
pub async fn trigger_visualization_transform(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    encryption: Data<EncryptionService>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != *user {
                return not_found("Visualization transform not found".to_string());
            }
        }
        Ok(None) => return not_found("Visualization transform not found".to_string()),
        Err(e) => {
            error!("Failed to fetch visualization transform: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transform: {}", e)
            }));
        }
    }

    match trigger_visualization_transform_scan(&postgres_pool, &nats_client, id, &user, &encryption)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "message": "Visualization transform triggered successfully"
        })),
        Err(e) => {
            error!("Failed to trigger visualization transform: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to trigger visualization transform: {}", e)
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
#[tracing::instrument(name = "get_visualization_transform_stats", skip(user, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualization_transform_stats(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != *user {
                return not_found("Visualization transform not found".to_string());
            }
        }
        Ok(None) => return not_found("Visualization transform not found".to_string()),
        Err(e) => {
            error!("Failed to fetch visualization transform: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transform: {}", e)
            }));
        }
    }

    // Get latest visualization
    let latest_visualization =
        match visualization_transforms::get_latest_visualization(&postgres_pool, id).await {
            Ok(visualization) => visualization,
            Err(e) => {
                error!("Failed to fetch latest visualization: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to fetch latest visualization: {}", e)
                }));
            }
        };

    // Get visualization counts
    let all_visualizations =
        match visualization_transforms::list_visualizations(&postgres_pool, id, 1000, 0).await {
            Ok(visualizations) => visualizations,
            Err(e) => {
                error!("Failed to fetch visualizations: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to fetch visualizations: {}", e)
                }));
            }
        };

    let total_runs = all_visualizations.len() as i64;
    let successful_runs = all_visualizations
        .iter()
        .filter(|v| v.status == "completed")
        .count() as i64;
    let failed_runs = all_visualizations
        .iter()
        .filter(|v| v.status == "failed")
        .count() as i64;

    let stats = VisualizationTransformStats {
        visualization_transform_id: id,
        latest_visualization,
        total_runs,
        successful_runs,
        failed_runs,
    };

    HttpResponse::Ok().json(stats)
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/visualizations",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID"),
        ("limit" = Option<i64>, Query, description = "Number of visualizations to return"),
        ("offset" = Option<i64>, Query, description = "Number of visualizations to skip"),
    ),
    responses(
        (status = 200, description = "List of visualizations", body = Vec<Visualization>),
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/visualizations")]
#[tracing::instrument(name = "get_visualizations", skip(user, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualizations(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    pagination: Query<PaginationParams>,
) -> impl Responder {
    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != *user {
                return not_found("Visualization transform not found".to_string());
            }
        }
        Ok(None) => return not_found("Visualization transform not found".to_string()),
        Err(e) => {
            error!("Failed to fetch visualization transform: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization transform: {}", e)
            }));
        }
    }

    match visualization_transforms::list_visualizations(
        &postgres_pool,
        id,
        pagination.limit,
        pagination.offset,
    )
    .await
    {
        Ok(visualizations) => HttpResponse::Ok().json(visualizations),
        Err(e) => {
            error!("Failed to fetch visualizations: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualizations: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/visualizations/{visualization_id}",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID"),
        ("visualization_id" = i32, Path, description = "Visualization ID"),
    ),
    responses(
        (status = 200, description = "Visualization details", body = Visualization),
        (status = 404, description = "Visualization not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/visualizations/{visualization_id}")]
#[tracing::instrument(name = "get_visualization", skip(user, postgres_pool), fields(visualization_transform_id = %path.0, visualization_id = %path.1))]
pub async fn get_visualization(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let (transform_id, visualization_id) = path.into_inner();

    // Get visualization with owner check - ensures the visualization belongs to a transform owned by the user
    match visualization_transforms::get_visualization_with_owner(
        &postgres_pool,
        visualization_id,
        &user,
    )
    .await
    {
        Ok(visualization) => {
            // Verify the visualization belongs to the specified transform
            if visualization.visualization_transform_id != transform_id {
                return not_found("Visualization not found for this transform".to_string());
            }
            events::resource_read(
                &user,
                ResourceType::Visualization,
                &visualization_id.to_string(),
            );
            HttpResponse::Ok().json(visualization)
        }
        Err(e) => {
            error!("Failed to fetch visualization for user {}: {}", *user, e);
            not_found("Visualization not found".to_string())
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/embedded-datasets/{id}/visualizations",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Embedded Dataset ID")
    ),
    responses(
        (status = 200, description = "List of visualization transforms for the embedded dataset", body = Vec<VisualizationTransform>),
        (status = 404, description = "Embedded dataset not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/embedded-datasets/{id}/visualizations")]
#[tracing::instrument(name = "get_visualizations_by_dataset", skip(user, postgres_pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_visualizations_by_dataset(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let embedded_dataset_id = path.into_inner();

    // Verify embedded dataset exists and belongs to user
    match embedded_datasets::get_embedded_dataset(&postgres_pool, &user, embedded_dataset_id).await
    {
        Ok(dataset) => {
            if dataset.owner != *user {
                return not_found("Embedded dataset not found".to_string());
            }
        }
        Err(e) => {
            error!("Failed to fetch embedded dataset: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch embedded dataset: {}", e)
            }));
        }
    }

    match visualization_transforms::get_visualization_transforms_by_embedded_dataset(
        &postgres_pool,
        embedded_dataset_id,
        &user,
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

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/visualizations/{visualization_id}/download",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID"),
        ("visualization_id" = i32, Path, description = "Visualization ID"),
    ),
    responses(
        (status = 200, description = "HTML file download", content_type = "text/html"),
        (status = 404, description = "Visualization not found or no HTML file available"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/visualizations/{visualization_id}/download")]
#[tracing::instrument(name = "download_visualization_html", skip(user, postgres_pool, s3_client), fields(visualization_transform_id = %path.0, visualization_id = %path.1))]
pub async fn download_visualization_html(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    s3_client: Data<aws_sdk_s3::Client>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let (transform_id, visualization_id) = path.into_inner();

    // Get visualization with owner check - this verifies both the visualization exists and belongs to the user's transform
    let visualization = match visualization_transforms::get_visualization_with_owner(
        &postgres_pool,
        visualization_id,
        &user,
    )
    .await
    {
        Ok(visualization) => {
            // Verify the visualization belongs to the specified transform
            if visualization.visualization_transform_id != transform_id {
                return not_found("Visualization not found for this transform".to_string());
            }
            visualization
        }
        Err(e) => {
            error!("Failed to fetch visualization for user {}: {}", *user, e);
            return not_found("Visualization not found".to_string());
        }
    };

    // Check if HTML file exists
    let html_s3_key = match visualization.html_s3_key {
        Some(key) => key,
        None => {
            return not_found("No HTML file available for this visualization".to_string());
        }
    };

    // Bucket name is derived from transform ID (same pattern as Python worker)
    let bucket = format!("visualizations-{}", transform_id);

    // Download the file from S3
    match crate::storage::rustfs::get_file_with_size_check(&s3_client, &bucket, &html_s3_key).await
    {
        Ok(file_data) => {
            let filename = format!("visualization-{}-{}.html", transform_id, visualization_id);
            HttpResponse::Ok()
                .content_type("text/html")
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", filename),
                ))
                .body(file_data)
        }
        Err(e) => {
            error!("Failed to download HTML file: {}", e);
            let error_msg = e.to_string();
            if error_msg.contains("exceeds maximum download limit") {
                bad_request(error_msg)
            } else {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to download HTML file: {}", e)
                }))
            }
        }
    }
}
#[utoipa::path(
    get,
    path = "/api/visualizations/recent",
    tag = "Visualizations",
    params(
        ("limit" = Option<i64>, Query, description = "Maximum number of visualizations to return (default: 5)")
    ),
    responses(
        (status = 200, description = "List of recent visualizations", body = Vec<Visualization>),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualizations/recent")]
#[tracing::instrument(name = "get_recent_visualizations", skip(user, postgres_pool))]
pub async fn get_recent_visualizations(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    query: Query<PaginationParams>,
) -> impl Responder {
    let limit = query.limit.clamp(1, 100);

    match visualization_transforms::get_recent_visualizations(&postgres_pool, &user, limit).await {
        Ok(visualizations) => HttpResponse::Ok().json(visualizations),
        Err(e) => {
            error!("Failed to fetch recent visualizations: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch recent visualizations: {}", e)
            }))
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct VisualizationSSEStreamQuery {
    /// Optional embedded_dataset_id to filter updates for a specific embedded dataset
    pub embedded_dataset_id: Option<i32>,
    /// Optional visualization_transform_id to filter updates for a specific transform
    pub visualization_transform_id: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/stream",
    tag = "Visualization Transforms",
    params(
        ("embedded_dataset_id" = Option<i32>, Query, description = "Optional embedded dataset ID to filter updates"),
        ("visualization_transform_id" = Option<i32>, Query, description = "Optional visualization transform ID to filter updates"),
    ),
    responses(
        (status = 200, description = "Server-Sent Events stream of visualization transform status updates", content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/stream")]
#[tracing::instrument(
    name = "stream_visualization_transform_status",
    skip(user, nats_client)
)]
pub async fn stream_visualization_transform_status(
    user: AuthenticatedUser,
    nats_client: Data<NatsClient>,
    query: Query<VisualizationSSEStreamQuery>,
) -> impl Responder {
    use actix_web::http::header;
    use futures_util::stream::StreamExt;
    use std::time::Duration;
    use tokio::time::interval;

    let owner = user.to_string();
    let nats = nats_client.get_ref().clone();
    let embedded_dataset_id_filter = query.embedded_dataset_id;
    let visualization_transform_id_filter = query.visualization_transform_id;

    // Create SSE stream
    let stream = async_stream::stream! {
        // Subscribe to visualization transform status updates
        // Subject format: transforms.visualization.status.{owner}.{embedded_dataset_id}.{transform_id}
        // Use wildcards for flexible filtering at subscription level
        let subject = match (embedded_dataset_id_filter, visualization_transform_id_filter) {
            (Some(embedded_dataset_id), Some(transform_id)) => {
                format!("transforms.visualization.status.{}.{}.{}", owner, embedded_dataset_id, transform_id)
            }
            (Some(embedded_dataset_id), None) => {
                format!("transforms.visualization.status.{}.{}.*", owner, embedded_dataset_id)
            }
            (None, Some(transform_id)) => {
                // Filter by transform_id across all embedded datasets
                format!("transforms.visualization.status.{}.*.{}", owner, transform_id)
            }
            (None, None) => {
                format!("transforms.visualization.status.{}.>", owner)
            }
        };

        let mut subscriber = match nats.subscribe(subject.clone()).await {
            Ok(sub) => sub,
            Err(e) => {
                error!("Failed to subscribe to NATS subject '{}': {}", subject, e);
                yield Err(actix_web::error::ErrorInternalServerError(e));
                return;
            }
        };

        info!("SSE client connected to visualization transforms stream (subject: {})", subject);

        // Send initial connection message
        yield Ok::<_, actix_web::Error>(actix_web::web::Bytes::from("event: connected\ndata: {\"status\":\"connected\"}\n\n"));

        // Heartbeat interval (30 seconds)
        let mut heartbeat = interval(Duration::from_secs(30));
        heartbeat.tick().await; // Skip first immediate tick

        loop {
            tokio::select! {
                // Handle NATS messages
                msg_result = subscriber.next() => {
                    match msg_result {
                        Some(msg) => {
                            match String::from_utf8(msg.payload.to_vec()) {
                                Ok(payload) => {
                                    yield Ok(actix_web::web::Bytes::from(format!("event: status\ndata: {}\n\n", payload)));
                                }
                                Err(e) => {
                                    error!("Failed to parse message payload: {}", e);
                                }
                            }
                        }
                        None => {
                            // Subscription closed
                            yield Ok(actix_web::web::Bytes::from("event: closed\ndata: {\"status\":\"subscription_closed\"}\n\n"));
                            break;
                        }
                    }
                }
                // Send heartbeat
                _ = heartbeat.tick() => {
                    yield Ok(actix_web::web::Bytes::from(": heartbeat\n\n"));
                }
            }
        }
    };

    HttpResponse::Ok()
        .insert_header(header::ContentType(mime::TEXT_EVENT_STREAM))
        .insert_header(header::CacheControl(vec![header::CacheDirective::NoCache]))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}
