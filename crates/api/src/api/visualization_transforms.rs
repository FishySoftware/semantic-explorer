use crate::auth::extract_username;
use crate::errors::{bad_request, not_found};
use crate::storage::postgres::{embedded_datasets, llms, visualization_transforms};
use crate::transforms::visualization::models::{
    CreateVisualizationTransform, UpdateVisualizationTransform, VisualizationTransform,
    VisualizationTransformRun, VisualizationTransformStats,
};
use crate::transforms::visualization::scanner::trigger_visualization_transform_scan;

use actix_web::web::{Data, Json, Path, Query};
use actix_web::{HttpResponse, Responder, delete, get, patch, post};
use actix_web_openidconnect::openid_middleware::Authenticated;
use async_nats::Client as NatsClient;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tracing::{error, info};

#[derive(Deserialize, Debug)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

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

    let id = path.into_inner();
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != username {
                return not_found("Visualization transform not found".to_string());
            }
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
#[tracing::instrument(name = "create_visualization_transform", skip(auth, postgres_pool, nats_client, body), fields(title = %body.title))]
pub async fn create_visualization_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    body: Json<CreateVisualizationTransform>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    // Verify embedded dataset exists and belongs to user
    match embedded_datasets::get_embedded_dataset(
        &postgres_pool,
        &username,
        body.embedded_dataset_id,
    )
    .await
    {
        Ok(dataset) => {
            if dataset.owner != username {
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
        match llms::get_llm(&postgres_pool, &username, llm_id).await {
            Ok(llm) => {
                if llm.owner != username {
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
        &username,
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
                &username,
            )
            .await
            {
                error!(
                    "Failed to trigger visualization transform scan for newly created transform {}: {}",
                    transform_id, e
                );
                // Don't fail the creation, just log the error
            }

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

    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != username {
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
        body.title.as_deref(),
        body.is_enabled,
        body.visualization_config.as_ref(),
    )
    .await
    {
        Ok(transform) => HttpResponse::Ok().json(transform),
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

    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != username {
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

    match visualization_transforms::delete_visualization_transform(&postgres_pool, id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
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
#[tracing::instrument(name = "trigger_visualization_transform", skip(auth, postgres_pool, nats_client), fields(visualization_transform_id = %path.as_ref()))]
pub async fn trigger_visualization_transform(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    nats_client: Data<NatsClient>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != username {
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

    match trigger_visualization_transform_scan(&postgres_pool, &nats_client, id, &username).await {
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

    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != username {
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

    // Get latest run
    let latest_run =
        match visualization_transforms::get_latest_visualization_run(&postgres_pool, id).await {
            Ok(run) => run,
            Err(e) => {
                error!("Failed to fetch latest run: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Failed to fetch latest run: {}", e)
                }));
            }
        };

    // Get run counts
    let all_runs = match visualization_transforms::list_visualization_runs(
        &postgres_pool,
        id,
        1000,
        0,
    )
    .await
    {
        Ok(runs) => runs,
        Err(e) => {
            error!("Failed to fetch runs: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch runs: {}", e)
            }));
        }
    };

    let total_runs = all_runs.len() as i64;
    let successful_runs = all_runs.iter().filter(|r| r.status == "completed").count() as i64;
    let failed_runs = all_runs.iter().filter(|r| r.status == "failed").count() as i64;

    let stats = VisualizationTransformStats {
        visualization_transform_id: id,
        latest_run,
        total_runs,
        successful_runs,
        failed_runs,
    };

    HttpResponse::Ok().json(stats)
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/runs",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID"),
        ("limit" = Option<i64>, Query, description = "Number of runs to return"),
        ("offset" = Option<i64>, Query, description = "Number of runs to skip"),
    ),
    responses(
        (status = 200, description = "List of visualization runs", body = Vec<VisualizationTransformRun>),
        (status = 404, description = "Visualization transform not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/runs")]
#[tracing::instrument(name = "get_visualization_runs", skip(auth, postgres_pool), fields(visualization_transform_id = %path.as_ref()))]
pub async fn get_visualization_runs(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
    pagination: Query<PaginationParams>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let id = path.into_inner();

    // Verify ownership
    match visualization_transforms::get_visualization_transform_by_id(&postgres_pool, id).await {
        Ok(Some(transform)) => {
            if transform.owner != username {
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

    match visualization_transforms::list_visualization_runs(
        &postgres_pool,
        id,
        pagination.limit,
        pagination.offset,
    )
    .await
    {
        Ok(runs) => HttpResponse::Ok().json(runs),
        Err(e) => {
            error!("Failed to fetch visualization runs: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to fetch visualization runs: {}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/visualization-transforms/{id}/runs/{run_id}",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID"),
        ("run_id" = i32, Path, description = "Run ID"),
    ),
    responses(
        (status = 200, description = "Visualization run details", body = VisualizationTransformRun),
        (status = 404, description = "Visualization run not found"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/runs/{run_id}")]
#[tracing::instrument(name = "get_visualization_run", skip(auth, postgres_pool), fields(visualization_transform_id = %path.0, run_id = %path.1))]
pub async fn get_visualization_run(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let (transform_id, run_id) = path.into_inner();

    // Get run with owner check - ensures the run belongs to a transform owned by the user
    match visualization_transforms::get_visualization_run_with_owner(
        &postgres_pool,
        run_id,
        &username,
    )
    .await
    {
        Ok(run) => {
            // Verify the run belongs to the specified transform
            if run.visualization_transform_id != transform_id {
                return not_found("Visualization run not found for this transform".to_string());
            }
            HttpResponse::Ok().json(run)
        }
        Err(e) => {
            error!(
                "Failed to fetch visualization run for user {}: {}",
                username, e
            );
            not_found("Visualization run not found".to_string())
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
#[tracing::instrument(name = "get_visualizations_by_dataset", skip(auth, postgres_pool), fields(embedded_dataset_id = %path.as_ref()))]
pub async fn get_visualizations_by_dataset(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let embedded_dataset_id = path.into_inner();

    // Verify embedded dataset exists and belongs to user
    match embedded_datasets::get_embedded_dataset(&postgres_pool, &username, embedded_dataset_id)
        .await
    {
        Ok(dataset) => {
            if dataset.owner != username {
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
        &username,
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
    path = "/api/visualization-transforms/{id}/runs/{run_id}/download",
    tag = "Visualization Transforms",
    params(
        ("id" = i32, Path, description = "Visualization Transform ID"),
        ("run_id" = i32, Path, description = "Run ID"),
    ),
    responses(
        (status = 200, description = "HTML file download", content_type = "text/html"),
        (status = 404, description = "Visualization run not found or no HTML file available"),
        (status = 401, description = "Unauthorized"),
    ),
)]
#[get("/api/visualization-transforms/{id}/runs/{run_id}/download")]
#[tracing::instrument(name = "download_visualization_html", skip(auth, postgres_pool, s3_client), fields(visualization_transform_id = %path.0, run_id = %path.1))]
pub async fn download_visualization_html(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    s3_client: Data<aws_sdk_s3::Client>,
    path: Path<(i32, i32)>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let (transform_id, run_id) = path.into_inner();

    // Get run with owner check - this verifies both the run exists and belongs to the user's transform
    let run = match visualization_transforms::get_visualization_run_with_owner(
        &postgres_pool,
        run_id,
        &username,
    )
    .await
    {
        Ok(run) => {
            // Verify the run belongs to the specified transform
            if run.visualization_transform_id != transform_id {
                return not_found("Visualization run not found for this transform".to_string());
            }
            run
        }
        Err(e) => {
            error!(
                "Failed to fetch visualization run for user {}: {}",
                username, e
            );
            return not_found("Visualization run not found".to_string());
        }
    };

    // Check if HTML file exists
    let html_s3_key = match run.html_s3_key {
        Some(key) => key,
        None => {
            return not_found("No HTML file available for this visualization run".to_string());
        }
    };

    // Get the bucket from environment
    let bucket = match std::env::var("AWS_S3_BUCKET") {
        Ok(bucket) => bucket,
        Err(_) => {
            error!("AWS_S3_BUCKET environment variable not set");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Storage configuration error"
            }));
        }
    };

    // Download the file from S3
    match crate::storage::rustfs::get_file_with_size_check(&s3_client, &bucket, &html_s3_key).await
    {
        Ok(file_data) => {
            let filename = format!("visualization-{}-{}.html", transform_id, run_id);
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
