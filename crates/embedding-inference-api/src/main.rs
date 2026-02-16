//! Inference API service entry point.
//!
//! Local AI inference service for Semantic Explorer providing embedding and reranking
//! capabilities using fastembed-rs ONNX models with CUDA GPU acceleration.

mod api;
mod config;
mod embedding;
mod errors;
mod models;
mod observability;
mod reranker;

use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware::Compress, web};
use anyhow::Result;
use dotenvy::dotenv;
use tracing::info;
use utoipa::OpenApi;
use utoipa_actix_web::AppExt;
use utoipa_swagger_ui::SwaggerUi;

use config::InferenceConfig;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Inference API",
        description = "Local AI inference service for embeddings and reranking with CUDA GPU acceleration",
        version = "1.0.0"
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "embedding", description = "Text embedding generation"),
        (name = "reranking", description = "Document reranking"),
        (name = "models", description = "Model discovery and listing")
    )
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Load configuration
    let mut config = InferenceConfig::from_env()?;

    // Initialize observability
    let prometheus = observability::init_observability(&config.observability)?;

    info!(
        hostname = %config.server.hostname,
        port = config.server.port,
        "Starting embedding-inference-api server with CUDA GPU acceleration (controlled by CUDA_VISIBLE_DEVICES)"
    );

    // Initialize NVML and resolve arena size before loading models.
    // If no explicit CUDA_ARENA_SIZE is set, auto-compute from GPU VRAM x pressure threshold
    // so the arena never grows past the rejection threshold.
    {
        use semantic_explorer_core::observability::gpu_monitor;
        gpu_monitor::init();
        config.models.resolve_effective_arena_size();
    }

    // Initialize queue configuration before loading models
    embedding::init_queue_config(
        config.models.max_queue_depth,
        config.models.queue_timeout_ms,
    );

    tokio::join!(
        embedding::init_cache(&config.models),
        reranker::init_cache(&config.models)
    );

    // Start GPU VRAM pressure monitoring (NVML-based, configurable threshold)
    embedding::spawn_gpu_pressure_monitor(config.models.gpu_pressure_threshold);

    info!(
        max_queue_depth = config.models.max_queue_depth,
        queue_timeout_ms = config.models.queue_timeout_ms,
        cuda_arena_size = ?config.models.cuda_arena_size.map(|s| format!("{}MB", s / (1024 * 1024))).unwrap_or_else(|| "unlimited".to_string()),
        cuda_arena_extend_strategy = ?config.models.cuda_arena_extend_strategy,
        "Model workers, queue, and GPU monitor initialized."
    );

    let model_config = web::Data::new(config.models.clone());
    let hostname = config.server.hostname.clone();
    let port = config.server.port;
    let cors_origins = config.server.cors_allowed_origins.clone();
    let tls_config = config.tls.clone();

    let server = HttpServer::new(move || {
        // Configure CORS
        let cors = if cors_origins.contains(&"*".to_string()) {
            Cors::permissive()
        } else {
            let mut cors = Cors::default()
                .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                .allowed_headers(vec![
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::AUTHORIZATION,
                ])
                .supports_credentials()
                .max_age(3600);
            for origin in &cors_origins {
                cors = cors.allowed_origin(origin);
            }
            cors
        };

        App::new()
            .wrap(prometheus.clone())
            .wrap(cors)
            .wrap(Compress::default())
            .app_data(model_config.clone())
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .service(api::health::health_live)
            .service(api::health::health_ready)
            .service(api::health::health_status)
            .service(api::embedding::list_embedders)
            .service(api::embedding::embed)
            .service(api::embedding::embed_batch)
            .service(api::reranking::list_rerankers)
            .service(api::reranking::rerank)
            .openapi_service(|api| {
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api/openapi.json", api)
            })
            .into_app()
    });

    // Bind server with optional TLS
    if tls_config.server_ssl_enabled {
        let cert_path = tls_config.server_cert_path.clone().ok_or_else(|| {
            anyhow::anyhow!("TLS_SERVER_CERT_PATH is required when SERVER_SSL_ENABLED=true")
        })?;
        let key_path = tls_config.server_key_path.clone().ok_or_else(|| {
            anyhow::anyhow!("TLS_SERVER_KEY_PATH is required when SERVER_SSL_ENABLED=true")
        })?;

        let rustls_config = semantic_explorer_core::tls::load_tls_config(&cert_path, &key_path)?;

        info!(
            hostname = %hostname,
            port = port,
            "Server running with TLS at https://{}:{}",
            hostname,
            port
        );

        server
            .bind_rustls_0_23((hostname.as_str(), port), rustls_config)?
            .run()
            .await?;
    } else {
        info!(
            hostname = %hostname,
            port = port,
            "Server running at http://{}:{}",
            hostname,
            port
        );

        server.bind((hostname.as_str(), port))?.run().await?;
    }

    Ok(())
}
