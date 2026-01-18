//! LLM Inference API service entry point.
//!
//! Local AI inference service for Semantic Explorer providing text generation,
//! streaming, and chat capabilities using mistral.rs with CUDA GPU acceleration.

mod api;
mod config;
mod errors;
mod llm;
mod models;
mod observability;

use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware::Compress, web};
use anyhow::Result;
use dotenvy::dotenv;
use tracing::info;
use utoipa::OpenApi;
use utoipa_actix_web::AppExt;
use utoipa_swagger_ui::SwaggerUi;

use config::LlmInferenceConfig;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "LLM Inference API",
        description = "Local AI inference service for text generation with CUDA GPU acceleration via mistral.rs",
        version = "1.0.0"
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "generation", description = "Text generation"),
        (name = "chat", description = "Chat completions"),
        (name = "models", description = "Model discovery and listing")
    )
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Load configuration
    let config = LlmInferenceConfig::from_env()?;

    // Initialize observability
    let prometheus = observability::init_observability(&config.observability)?;

    info!(
        hostname = %config.server.hostname,
        port = config.server.port,
        "Starting llm-inference-api server with CUDA GPU acceleration (controlled by CUDA_VISIBLE_DEVICES)"
    );

    info!(
        allowed_models = ?config.models.allowed_models,
        count = config.models.allowed_models.len(),
        "Allowed LLM models configured"
    );

    // Initialize model cache
    llm::init_cache(&config.models).await;

    info!("Model cache initialized.");

    let model_config = web::Data::new(config.models.clone());
    let gen_config = web::Data::new(config.generation.clone());
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
            .app_data(gen_config.clone())
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            // Health endpoints
            .service(api::health::health_live)
            .service(api::health::health_ready)
            // Model listing
            .service(api::generation::list_llms)
            // Generation endpoints
            .service(api::generation::generate)
            .service(api::streaming::generate_stream)
            .service(api::chat::chat_completion)
            // Swagger UI
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
