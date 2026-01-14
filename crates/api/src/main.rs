mod api;
mod audit;
mod audit_worker;
mod auth;
mod chat;
mod collections;
mod datasets;
mod embedded_datasets;
mod embedders;
mod embedding;
mod errors;
mod llms;
mod middleware;
mod observability;
mod search;
mod storage;
mod transforms;
mod validation;

use actix_cors::Cors;
use actix_multipart::form::MultipartFormConfig;
use actix_web::{
    App, HttpServer,
    http::header,
    middleware::{Compress, DefaultHeaders},
    web,
};
use anyhow::Result;
use dotenvy::dotenv;
use semantic_explorer_core::config::AppConfig;
use semantic_explorer_core::encryption::EncryptionService;
use std::path::PathBuf;
use tracing::info;
use utoipa::OpenApi;
use utoipa_actix_web::AppExt;
use utoipa_swagger_ui::SwaggerUi;

#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(OpenApi)]
#[openapi(info(title = "Semantic Explorer"))]
struct ApiDoc;

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Load centralized configuration - fail fast if required config is missing
    let config = AppConfig::from_env()?;

    // Initialize HTTP client with TLS configuration
    semantic_explorer_core::http_client::initialize(&config.tls)?;

    // Initialize encryption service for secrets (API keys)
    let encryption_service = EncryptionService::from_env().map_err(|e| {
        eprintln!(
            "Warning: Encryption service not initialized: {}. API keys will NOT be encrypted.",
            e
        );
        eprintln!("To enable encryption, set ENCRYPTION_MASTER_KEY environment variable");
        eprintln!("Generate a key with: echo $(openssl rand -hex 32)");
        e
    })?;

    let prometheus = observability::init_observability()?;
    let hostname = config.server.hostname.clone();
    let port = config.server.port;
    let address = format!("http://{}:{}", hostname, port);
    let static_files_directory = PathBuf::from(config.server.static_files_dir.clone());

    // Graceful shutdown timeout from config or default 30 seconds
    let shutdown_timeout = config.server.shutdown_timeout_secs.unwrap_or(30);

    let s3_client = storage::rustfs::initialize_client().await?;
    let qdrant_client = storage::qdrant::initialize_client(&config.qdrant).await?;
    let postgres_pool = storage::postgres::initialize_pool(&config.database).await?;
    let openid_client = auth::oidc::initialize_client(format!("{address}/auth_callback")).await?;
    let nats_client = async_nats::connect(&config.nats.url).await?;

    // Initialize Redis cluster client for rate limiting
    let redis_client = redis::cluster::ClusterClient::new(config.redis.cluster_nodes.clone())?;

    // Test Redis connection
    match redis_client.get_async_connection().await {
        Ok(_) => {
            info!("Redis cluster connection established");
        }
        Err(e) => {
            if config.rate_limit.enabled {
                return Err(anyhow::anyhow!(
                    "Failed to connect to Redis cluster (required for rate limiting): {}",
                    e
                ));
            } else {
                tracing::warn!(
                    "Redis cluster unavailable but rate limiting is disabled: {}",
                    e
                );
            }
        }
    }

    // Keep references for graceful shutdown
    let nats_shutdown = nats_client.clone();
    let postgres_shutdown = postgres_pool.clone();
    let redis_shutdown = redis_client.clone();

    semantic_explorer_core::nats::initialize_jetstream(&nats_client, &config.nats).await?;

    // Start session cleanup background job (runs every hour)
    let session_cleanup_handle = start_session_cleanup_job(postgres_pool.clone());

    // Start audit event consumer worker
    let audit_consumer_handle = {
        let nats = nats_client.clone();
        let db = postgres_pool.clone();
        tokio::spawn(async move {
            if let Err(e) = audit_worker::start_audit_consumer(nats, db).await {
                tracing::error!(error = %e, "Audit consumer exited with error");
            }
        })
    };

    transforms::listeners::start_result_listeners(
        postgres_pool.clone(),
        s3_client.clone(),
        nats_client.clone(),
        encryption_service.clone(),
    )
    .await?;

    let cors_origins = config.server.cors_allowed_origins.clone();

    // Start scanners for each transform type
    let collection_scanner_handle = transforms::collection::scanner::initialize_scanner(
        postgres_pool.clone(),
        nats_client.clone(),
        s3_client.clone(),
        encryption_service.clone(),
    );

    let dataset_scanner_handle = transforms::dataset::scanner::initialize_scanner(
        postgres_pool.clone(),
        nats_client.clone(),
        s3_client.clone(),
        encryption_service.clone(),
    );

    // Initialize audit event infrastructure (database and NATS)
    audit::events::init(postgres_pool.clone(), nats_client.clone());

    // Initialize rate limiting metrics
    let rate_limit_metrics = middleware::RateLimitMetrics::new(&prometheus.registry)
        .map_err(|e| anyhow::anyhow!("Failed to initialize rate limit metrics: {}", e))?;

    // Clone config and clients for use in server closure
    let rate_limit_config = config.rate_limit.clone();

    let server = HttpServer::new(move || {
        // Build CORS configuration based on allowed origins
        let cors = if cors_origins.is_empty() {
            Cors::default()
                .allowed_origin(&address)
                .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
                .allowed_headers(vec![
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::ACCEPT,
                ])
                .supports_credentials()
                .max_age(3600)
        } else {
            // Production: use configured origins
            let mut cors = Cors::default()
                .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
                .allowed_headers(vec![
                    header::AUTHORIZATION,
                    header::CONTENT_TYPE,
                    header::ACCEPT,
                ])
                .supports_credentials()
                .max_age(3600);
            for origin in &cors_origins {
                cors = cors.allowed_origin(origin);
            }
            cors
        };

        // Create rate limiter (always wrap, but it can be disabled via config)
        let rate_limiter = middleware::RateLimitMiddleware::new(
            redis_client.clone(),
            rate_limit_config.clone(),
            rate_limit_metrics.clone(),
        );

        // Create idempotency middleware for transform endpoints
        let idempotency_config = middleware::IdempotencyConfig::new(redis_client.clone())
            .with_prefix("idempotency:transforms".to_string())
            .with_ttl(86400); // 24 hours
        let idempotency = middleware::IdempotencyMiddleware::new(idempotency_config);

        // Create session activity tracking middleware
        let session_activity = middleware::SessionActivityMiddleware::new(postgres_pool.clone());

        // Security headers middleware
        let security_headers = DefaultHeaders::new()
            .add(("X-Content-Type-Options", "nosniff"))
            .add(("X-Frame-Options", "DENY"))
            .add(("X-XSS-Protection", "1; mode=block"))
            .add(("Referrer-Policy", "strict-origin-when-cross-origin"))
            .add((
                "Permissions-Policy",
                "geolocation=(), microphone=(), camera=()",
            ));

        App::new()
            .wrap(prometheus.clone())
            .wrap(rate_limiter)
            .wrap(idempotency)
            .wrap(cors)
            .wrap(security_headers)
            .wrap(Compress::default())
            .wrap(openid_client.get_middleware())
            .wrap(session_activity)
            .configure(openid_client.configure_open_id())
            .app_data(web::Data::new(s3_client.clone()))
            .app_data(web::Data::new(qdrant_client.clone()))
            .app_data(web::Data::new(postgres_pool.clone()))
            .app_data(web::Data::new(nats_client.clone()))
            .app_data(web::Data::new(encryption_service.clone()))
            .app_data(
                MultipartFormConfig::default()
                    .total_limit(1024 * 1024 * 1024) // 1GB max per upload (in memory)
                    .memory_limit(1024 * 1024 * 1024), // 1GB in memory (no temp files)
            )
            .app_data(web::Data::new(static_files_directory.clone()))
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .service(api::collections::get_collections)
            .service(api::collections::search_collections)
            .service(api::collections::create_collections)
            .service(api::collections::update_collections)
            .service(api::collections::delete_collections)
            .service(api::collections::upload_to_collection)
            .service(api::collections::list_collection_files)
            .service(api::collections::download_collection_file)
            .service(api::collections::delete_collection_file)
            .service(api::datasets::get_datasets)
            .service(api::datasets::get_datasets_embedders)
            .service(api::datasets::get_dataset)
            .service(api::datasets::create_dataset)
            .service(api::datasets::update_dataset)
            .service(api::datasets::delete_dataset)
            .service(api::datasets::get_dataset_items)
            .service(api::datasets::get_dataset_items_summary)
            .service(api::datasets::get_dataset_item_chunks)
            .service(api::datasets::upload_to_dataset)
            .service(api::datasets::delete_dataset_item)
            .service(api::embedders::get_embedders)
            .service(api::embedders::get_embedder)
            .service(api::embedders::create_embedder)
            .service(api::embedders::update_embedder)
            .service(api::embedders::delete_embedder)
            .service(api::embedders::test_embedder)
            .service(api::llms::get_llms)
            .service(api::llms::get_llm)
            .service(api::llms::create_llm)
            .service(api::llms::update_llm)
            .service(api::llms::delete_llm)
            .service(api::marketplace::get_public_collections)
            .service(api::marketplace::get_recent_public_collections)
            .service(api::marketplace::get_public_datasets)
            .service(api::marketplace::get_recent_public_datasets)
            .service(api::marketplace::get_public_embedders)
            .service(api::marketplace::get_recent_public_embedders)
            .service(api::marketplace::get_public_llms)
            .service(api::marketplace::get_recent_public_llms)
            .service(api::marketplace::grab_collection)
            .service(api::marketplace::grab_dataset)
            .service(api::marketplace::grab_embedder)
            .service(api::marketplace::grab_llm)
            .service(api::search::search)
            .service(api::collection_transforms::get_collection_transforms)
            .service(api::collection_transforms::stream_collection_transform_status)
            .service(api::collection_transforms::get_collection_transform)
            .service(api::collection_transforms::create_collection_transform)
            .service(api::collection_transforms::update_collection_transform)
            .service(api::collection_transforms::delete_collection_transform)
            .service(api::collection_transforms::trigger_collection_transform)
            .service(api::collection_transforms::get_collection_transform_stats)
            .service(api::collection_transforms::get_processed_files)
            .service(api::collection_transforms::get_collection_transforms_for_collection)
            .service(api::dataset_transforms::get_dataset_transforms)
            .service(api::dataset_transforms::stream_dataset_transform_status)
            .service(api::dataset_transforms::get_dataset_transform)
            .service(api::dataset_transforms::create_dataset_transform)
            .service(api::dataset_transforms::update_dataset_transform)
            .service(api::dataset_transforms::delete_dataset_transform)
            .service(api::dataset_transforms::trigger_dataset_transform)
            .service(api::dataset_transforms::get_dataset_transform_stats)
            .service(api::dataset_transforms::get_dataset_transform_detailed_stats)
            .service(api::dataset_transforms::get_dataset_transform_batches)
            .service(api::dataset_transforms::get_dataset_transform_batch)
            .service(api::dataset_transforms::get_dataset_transform_batch_stats)
            .service(api::dataset_transforms::get_dataset_transforms_for_dataset)
            .service(api::embedded_datasets::get_embedded_datasets)
            .service(api::embedded_datasets::get_embedded_dataset)
            .service(api::embedded_datasets::delete_embedded_dataset)
            .service(api::embedded_datasets::update_embedded_dataset)
            .service(api::embedded_datasets::get_embedded_dataset_stats)
            .service(api::embedded_datasets::get_batch_embedded_dataset_stats)
            .service(api::embedded_datasets::get_embedded_dataset_points)
            .service(api::embedded_datasets::get_point_vector)
            .service(api::embedded_datasets::get_processed_batches)
            .service(api::embedded_datasets::get_embedded_datasets_for_dataset)
            .service(api::visualization_transforms::get_visualization_transforms)
            .service(api::visualization_transforms::stream_visualization_transform_status)
            .service(api::visualization_transforms::get_visualization_transform)
            .service(api::visualization_transforms::create_visualization_transform)
            .service(api::visualization_transforms::update_visualization_transform)
            .service(api::visualization_transforms::delete_visualization_transform)
            .service(api::visualization_transforms::trigger_visualization_transform)
            .service(api::visualization_transforms::get_visualization_transform_stats)
            .service(api::visualization_transforms::get_visualizations)
            .service(api::visualization_transforms::get_visualization)
            .service(api::visualization_transforms::download_visualization_html)
            .service(api::visualization_transforms::get_visualizations_by_dataset)
            .service(api::visualization_transforms::get_recent_visualizations)
            .service(api::chat::create_chat_session)
            .service(api::chat::get_chat_sessions)
            .service(api::chat::get_chat_session)
            .service(api::chat::delete_chat_session)
            .service(api::chat::get_chat_messages)
            .service(api::chat::send_chat_message)
            .service(api::chat::stream_chat_message)
            .service(api::chat::regenerate_chat_message)
            .service(api::sessions::list_sessions)
            .service(api::sessions::revoke_session)
            .service(api::sessions::revoke_all_sessions)
            .openapi_service(|api| {
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api/openapi.json", api)
            })
            .into_app()
            // Health check endpoints (must be outside OpenAPI to avoid auth)
            .service(api::health::health)
            .service(api::health::liveness)
            .service(api::health::readiness)
            .service(api::base)
            .service(api::index)
            .service(api::pages)
            .service(api::get_current_user)
    });

    // Bind server with optional TLS
    let result = if config.tls.server_ssl_enabled {
        let cert_path = config.tls.server_cert_path.clone().ok_or_else(|| {
            anyhow::anyhow!("server_cert_path is required when SERVER_SSL_ENABLED=true")
        })?;
        let key_path = config.tls.server_key_path.clone().ok_or_else(|| {
            anyhow::anyhow!("server_key_path is required when SERVER_SSL_ENABLED=true")
        })?;

        let rustls_config = load_tls_config(&cert_path, &key_path)?;

        info!("server running at https://{}:{}", hostname, port);
        server
            .bind_rustls_0_23((hostname, port), rustls_config)?
            .shutdown_timeout(shutdown_timeout)
            .run()
            .await
    } else {
        info!("server running at http://{}:{}", hostname, port);
        server
            .bind((hostname, port))?
            .shutdown_timeout(shutdown_timeout)
            .run()
            .await
    };

    result?;

    info!("Shutting down gracefully...");

    // Stop background scanners
    collection_scanner_handle.abort();
    dataset_scanner_handle.abort();
    session_cleanup_handle.abort();
    audit_consumer_handle.abort();

    // Drain NATS client - flush pending messages
    if let Err(e) = nats_shutdown.drain().await {
        tracing::warn!(error = %e, "Failed to drain NATS client");
    }

    // Close database pool
    postgres_shutdown.close().await;

    // Close Redis cluster connections (drop handles cleanup automatically)
    drop(redis_shutdown);

    info!("Server shutdown complete");

    Ok(())
}

/// Start a background job that periodically cleans up expired sessions
/// Runs every hour and revokes sessions that have passed their expiration time
fn start_session_cleanup_job(pool: sqlx::Pool<sqlx::Postgres>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Run every hour
        loop {
            interval.tick().await;

            match storage::postgres::auth::cleanup_expired_sessions(&pool).await {
                Ok(count) => {
                    if count > 0 {
                        info!(
                            "Session cleanup job completed: revoked {} expired sessions",
                            count
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("Session cleanup job failed: {}", e);
                }
            }
        }
    })
}

/// Load rustls configuration from certificate and key files
fn load_tls_config(cert_path: &str, key_path: &str) -> Result<rustls::ServerConfig> {
    use std::fs;

    // Load certificate
    let cert_contents = fs::read_to_string(cert_path)
        .map_err(|e| anyhow::anyhow!("Failed to read certificate file {}: {}", cert_path, e))?;

    let cert_pem = pem::parse(&cert_contents)
        .map_err(|e| anyhow::anyhow!("Failed to parse certificate PEM: {}", e))?;

    if cert_pem.tag() != "CERTIFICATE" {
        return Err(anyhow::anyhow!(
            "Invalid certificate file: expected CERTIFICATE tag, got {}",
            cert_pem.tag()
        ));
    }

    let cert_der = cert_pem.contents().to_vec();
    let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der)];

    // Load private key
    let key_contents = fs::read_to_string(key_path)
        .map_err(|e| anyhow::anyhow!("Failed to read key file {}: {}", key_path, e))?;

    let key_pem = pem::parse(&key_contents)
        .map_err(|e| anyhow::anyhow!("Failed to parse private key PEM: {}", e))?;

    let key_der = key_pem.contents().to_vec();

    // Support multiple key formats: PKCS#8, RSA, and EC
    let private_key = match key_pem.tag() {
        "PRIVATE KEY" => {
            // PKCS#8 format
            rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                key_der,
            ))
        }
        "RSA PRIVATE KEY" => {
            // PKCS#1 RSA format
            rustls::pki_types::PrivateKeyDer::Pkcs1(rustls::pki_types::PrivatePkcs1KeyDer::from(
                key_der,
            ))
        }
        "EC PRIVATE KEY" => {
            // SEC1 EC format
            rustls::pki_types::PrivateKeyDer::Sec1(rustls::pki_types::PrivateSec1KeyDer::from(
                key_der,
            ))
        }
        tag => {
            return Err(anyhow::anyhow!(
                "Unsupported private key format: {}. Expected PRIVATE KEY, RSA PRIVATE KEY, or EC PRIVATE KEY",
                tag
            ));
        }
    };

    // Build server config
    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .map_err(|e| anyhow::anyhow!("Invalid certificate or key: {}", e))?;

    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(server_config)
}
