mod api;
mod auth;
mod chat;
mod collections;
mod datasets;
mod embedded_datasets;
mod embedders;
mod embedding;
mod llms;
mod observability;
mod search;
mod storage;
mod transforms;
mod visualizations;

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

    // Start scanners for each transform type
    let collection_scanner_handle = transforms::collection::initialize_scanner(
        postgres_pool.clone(),
        nats_client.clone(),
        s3_client.clone(),
    );

    let dataset_scanner_handle = transforms::dataset::initialize_scanner(
        postgres_pool.clone(),
        nats_client.clone(),
        s3_client.clone(),
    );

    let visualization_scanner_handle =
        transforms::visualization::initialize_scanner(postgres_pool.clone(), nats_client.clone());

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
            .service(api::collection_transforms::get_collection_transform)
            .service(api::collection_transforms::create_collection_transform)
            .service(api::collection_transforms::update_collection_transform)
            .service(api::collection_transforms::delete_collection_transform)
            .service(api::collection_transforms::trigger_collection_transform)
            .service(api::collection_transforms::get_collection_transform_stats)
            .service(api::collection_transforms::get_processed_files)
            .service(api::collection_transforms::get_collection_transforms_for_collection)
            .service(api::dataset_transforms::get_dataset_transforms)
            .service(api::dataset_transforms::get_dataset_transform)
            .service(api::dataset_transforms::create_dataset_transform)
            .service(api::dataset_transforms::update_dataset_transform)
            .service(api::dataset_transforms::delete_dataset_transform)
            .service(api::dataset_transforms::trigger_dataset_transform)
            .service(api::dataset_transforms::get_dataset_transform_stats)
            .service(api::dataset_transforms::get_dataset_transforms_for_dataset)
            .service(api::embedded_datasets::get_embedded_datasets)
            .service(api::embedded_datasets::get_embedded_dataset)
            .service(api::embedded_datasets::delete_embedded_dataset)
            .service(api::embedded_datasets::get_embedded_dataset_stats)
            .service(api::embedded_datasets::get_processed_batches)
            .service(api::embedded_datasets::get_embedded_datasets_for_dataset)
            .service(api::visualization_transforms::get_visualization_transforms)
            .service(api::visualization_transforms::get_visualization_transform)
            .service(api::visualization_transforms::create_visualization_transform)
            .service(api::visualization_transforms::update_visualization_transform)
            .service(api::visualization_transforms::delete_visualization_transform)
            .service(api::visualization_transforms::trigger_visualization_transform)
            .service(api::visualization_transforms::get_visualization_transform_stats)
            .service(api::visualization_transforms::get_visualization_points)
            .service(api::visualization_transforms::get_visualization_topics)
            .service(api::visualization_transforms::get_visualizations_for_embedded_dataset)
            .service(api::visualizations::get_visualization_points)
            .service(api::visualizations::get_visualization_topics)
            .service(api::chat::create_chat_session)
            .service(api::chat::get_chat_sessions)
            .service(api::chat::get_chat_session)
            .service(api::chat::delete_chat_session)
            .service(api::chat::get_chat_messages)
            .service(api::chat::send_chat_message)
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

    collection_scanner_handle.abort();
    dataset_scanner_handle.abort();
    visualization_scanner_handle.abort();

    info!("Server shutdown");

    Ok(())
}
