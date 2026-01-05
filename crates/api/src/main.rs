mod api;
mod auth;
mod collections;
mod datasets;
mod embedders;
mod observability;
mod storage;
mod transforms;

use actix_cors::Cors;
use actix_multipart::form::MultipartFormConfig;
use actix_web::{App, HttpServer, http::header, middleware::Compress, web};
use anyhow::Result;
use dotenvy::dotenv;
use std::{env, path::PathBuf};
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

    let prometheus = observability::init_observability()?;
    let hostname = env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()?;
    let address = format!("http://{}:{}", hostname, port);
    let static_files_directory = PathBuf::from(
        env::var("STATIC_FILES_DIR").unwrap_or_else(|_| "./semantic-explorer-ui/".to_string()),
    );
    let s3_client = storage::rustfs::initialize_client().await?;
    let qdrant_client = storage::qdrant::initialize_client().await?;
    let postgres_pool = storage::postgres::initialize_pool().await?;
    let openid_client = auth::oidc::initialize_client(format!("{address}/auth_callback")).await?;
    let nats_client = async_nats::connect(
        &env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string()),
    )
    .await?;

    transforms::listeners::start_result_listeners(
        postgres_pool.clone(),
        s3_client.clone(),
        nats_client.clone(),
    )
    .await?;

    let collection_scanner_handle = transforms::scanner::initialize_collection_scanner(
        postgres_pool.clone(),
        nats_client.clone(),
        s3_client.clone(),
    );

    info!("server running at {address}");

    HttpServer::new(move || {
        App::new()
            .wrap(prometheus.clone())
            .wrap(
                Cors::default()
                    .allowed_origin(&address)
                    .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE"])
                    .allowed_headers(vec![header::AUTHORIZATION, header::CONTENT_TYPE])
                    .max_age(3600),
            )
            .wrap(Compress::default())
            .wrap(openid_client.get_middleware())
            .configure(openid_client.configure_open_id())
            .app_data(web::Data::new(s3_client.clone()))
            .app_data(web::Data::new(qdrant_client.clone()))
            .app_data(web::Data::new(postgres_pool.clone()))
            .app_data(web::Data::new(nats_client.clone()))
            .app_data(
                MultipartFormConfig::default()
                    .total_limit(1024 * 1024 * 1024) // 1GB total
                    .memory_limit(1024 * 1024 * 1024), // 1GB in memory
            )
            .app_data(web::Data::new(static_files_directory.clone()))
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .service(api::collections::get_collections)
            .service(api::collections::create_collections)
            .service(api::collections::delete_collections)
            .service(api::collections::upload_to_collection)
            .service(api::collections::list_collection_files)
            .service(api::collections::download_collection_file)
            .service(api::collections::delete_collection_file)
            .service(api::datasets::get_datasets)
            .service(api::datasets::create_dataset)
            .service(api::datasets::update_dataset)
            .service(api::datasets::delete_dataset)
            .service(api::datasets::get_dataset_items)
            .service(api::datasets::upload_to_dataset)
            .service(api::datasets::delete_dataset_item)
            .service(api::datasets::get_datasets_embedders)
            .service(api::embedders::get_embedders)
            .service(api::embedders::get_embedder)
            .service(api::embedders::create_embedder)
            .service(api::embedders::update_embedder)
            .service(api::embedders::delete_embedder)
            .service(api::embedders::test_embedder)
            .service(api::search::search)
            .service(api::transforms::get_transforms)
            .service(api::transforms::get_transform)
            .service(api::transforms::create_transform)
            .service(api::transforms::update_transform)
            .service(api::transforms::delete_transform)
            .service(api::transforms::get_transform_stats)
            .service(api::transforms::get_processed_files)
            .service(api::transforms::trigger_transform)
            .openapi_service(|api| {
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api/openapi.json", api)
            })
            .into_app()
            .service(api::health)
            .service(api::base)
            .service(api::index)
            .service(api::pages)
            .service(api::get_current_user)
    })
    .bind((hostname, port))?
    .run()
    .await?;

    collection_scanner_handle.await.abort();

    info!("Server shutdown");

    Ok(())
}
