use actix_multipart::form::MultipartForm;
use actix_web::{
    HttpResponse, Responder, delete, get, post,
    web::{self, Data, Json, Path},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use aws_sdk_s3::Client;
use sqlx::{Pool, Postgres};
use tracing::error;
use uuid::Uuid;

use crate::{
    auth::extract_username,
    collections::models::{
        Collection, CollectionSearchQuery, CollectionUpload, CollectionUploadResponse,
        CreateCollection, FileListQuery, PaginatedCollections,
    },
    storage::{
        self,
        postgres::collections,
        rustfs::{DocumentUpload, PaginatedFiles, delete_file, upload_document},
    },
};

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Collection>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[get("/api/collections")]
#[tracing::instrument(name = "get_collections", skip(auth, postgres_pool))]
pub(crate) async fn get_collections(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    match collections::get_collections(&postgres_pool.into_inner(), &username).await {
        Ok(collections) => HttpResponse::Ok().json(collections),
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch collections");
            HttpResponse::InternalServerError().body(format!("error fetching collections: {e:?}"))
        }
    }
}

#[utoipa::path(
    params(
        ("q" = Option<String>, Query, description = "Search query for title, details, or tags"),
        ("limit" = Option<i64>, Query, description = "Maximum number of results to return (default: 100)"),
        ("offset" = Option<i64>, Query, description = "Number of results to skip (default: 0)"),
    ),
    responses(
        (status = 200, description = "OK", body = PaginatedCollections),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[get("/api/collections/search")]
#[tracing::instrument(name = "search_collections", skip(auth, postgres_pool), fields(query = ?query.q, limit = %query.limit, offset = %query.offset))]
pub(crate) async fn search_collections(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    web::Query(query): web::Query<CollectionSearchQuery>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    let postgres_pool = postgres_pool.into_inner();

    let result = if let Some(search_query) = &query.q {
        collections::search_collections(
            &postgres_pool,
            &username,
            search_query,
            query.limit,
            query.offset,
        )
        .await
    } else {
        collections::get_collections_paginated(&postgres_pool, &username, query.limit, query.offset)
            .await
    };

    match result {
        Ok((collections, total_count)) => {
            let response = PaginatedCollections {
                collections,
                total_count,
                limit: query.limit,
                offset: query.offset,
            };
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to search collections");
            HttpResponse::InternalServerError().body(format!("error searching collections: {e:?}"))
        }
    }
}

#[utoipa::path(
    request_body = CreateCollection,
    responses(
        (status = 201, description = "Created"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[post("/api/collections")]
#[tracing::instrument(name = "create_collection", skip(auth, s3_client, postgres_pool, create_collection), fields(collection_title = %create_collection.title))]
pub(crate) async fn create_collections(
    auth: Authenticated,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    Json(create_collection): Json<CreateCollection>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let s3_client = s3_client.into_inner();
    let bucket = Uuid::new_v4().to_string();

    if let Err(e) = s3_client.create_bucket().bucket(&bucket).send().await {
        return HttpResponse::InternalServerError().body(format!(
            "error creating collection bucket '{bucket}' due to: {e:?}"
        ));
    }

    let collection = match collections::create_collection(
        &postgres_pool.into_inner(),
        &create_collection.title,
        create_collection.details.as_deref(),
        &username,
        &bucket,
        &create_collection.tags,
    )
    .await
    {
        Ok(collection) => collection,
        Err(e) => {
            if let Err(e) = s3_client.delete_bucket().bucket(&bucket).send().await {
                error!("error deleting collection bucket '{bucket}' due to: {e:?}");
            }
            return HttpResponse::InternalServerError()
                .body(format!("error creating collection due to: {e:?}"));
        }
    };

    HttpResponse::Created().json(collection)
}

#[utoipa::path(
    responses(
        (status = 200, description = "Ok"),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[delete("/api/collections/{collection_id}")]
#[tracing::instrument(name = "delete_collection", skip(auth, s3_client, postgres_pool), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn delete_collections(
    auth: Authenticated,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let postgres_pool = postgres_pool.into_inner();
    let collection_id = collection_id.into_inner();

    let collection =
        match collections::get_collection(&postgres_pool, &username, collection_id).await {
            Ok(collection) => collection,
            Err(_) => {
                return HttpResponse::NotFound().finish();
            }
        };

    if let Err(e) = s3_client
        .into_inner()
        .delete_bucket()
        .bucket(&collection.bucket)
        .send()
        .await
    {
        error!("error deleting collection bucket '{collection_id}' due to: {e:?}");
    }

    match collections::delete_collection(&postgres_pool, collection_id, &username).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete collection");
            HttpResponse::InternalServerError()
                .body(format!("error deleting collection due to: {e:?}"))
        }
    }
}

#[utoipa::path(
    request_body(content = CollectionUpload, content_type = "multipart/form-data"),
    params(
        ("collection_id", description = "Collection ID"),
     ),
    responses(
        (status = 201, description = "Created", body = CollectionUploadResponse),
        (status = 400, description = "Bad Request (collection does not exist)"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[post("/api/collections/{collection_id}/files")]
#[tracing::instrument(name = "upload_to_collection", skip(auth, s3_client, postgres_pool, payload), fields(collection_id = %collection_id.as_ref(), file_count = payload.files.len()))]
pub(crate) async fn upload_to_collection(
    auth: Authenticated,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    MultipartForm(payload): MultipartForm<CollectionUpload>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => {
            tracing::error!("Failed to extract username from authentication");
            return e;
        }
    };
    let s3_client = s3_client.into_inner();
    let postgres_pool = postgres_pool.into_inner();
    let collection_id = collection_id.into_inner();

    let collection =
        match collections::get_collection(&postgres_pool, &username, collection_id).await {
            Ok(collection) => collection,
            Err(e) => {
                tracing::error!(
                    collection_id = collection_id,
                    username = %username,
                    error = %e,
                    "Collection not found or access denied"
                );
                return HttpResponse::BadRequest()
                    .body(format!("collection '{collection_id}' does not exists"));
            }
        };

    let mut completed = Vec::with_capacity(payload.files.len());
    let mut failed = Vec::new();

    for (idx, file_bytes) in payload.files.iter().enumerate() {
        let file_name = file_bytes
            .file_name
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("file_{}", idx));
        let mime_type = mime_guess::from_path(&file_name)
            .first_or_octet_stream()
            .to_string();

        let document = DocumentUpload {
            collection_id: collection.bucket.clone(),
            name: file_name.clone(),
            content: file_bytes.data.to_vec(),
            mime_type: mime_type.clone(),
        };
        if let Err(e) = upload_document(&s3_client, document).await {
            failed.push(file_name.clone());
            tracing::error!(
                file_name = %file_name,
                error = %e,
                "Failed to upload file to S3"
            );
            continue;
        }

        completed.push(file_name);
    }

    HttpResponse::Ok().json(CollectionUploadResponse { completed, failed })
}

#[utoipa::path(
    params(
        ("collection_id", description = "Collection ID"),
        ("page_size" = Option<usize>, Query, description = "Number of items per page (default: 10)"),
        ("continuation_token" = Option<String>, Query, description = "Continuation token for cursor-based pagination"),
    ),
    responses(
        (status = 200, description = "OK", body = PaginatedFiles),
        (status = 404, description = "Collection not found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[get("/api/collections/{collection_id}/files")]
#[tracing::instrument(name = "list_collection_files", skip(auth, s3_client, postgres_pool, query), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn list_collection_files(
    auth: Authenticated,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    web::Query(query): web::Query<FileListQuery>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let collection_id = collection_id.into_inner();
    let collection =
        match collections::get_collection(&postgres_pool.into_inner(), &username, collection_id)
            .await
        {
            Ok(collection) => collection,
            Err(_) => {
                return HttpResponse::NotFound()
                    .body(format!("collection '{collection_id}' not found"));
            }
        };

    let s3_client = s3_client.into_inner();

    match storage::rustfs::list_files(
        &s3_client,
        &collection.bucket,
        query.page_size,
        query.continuation_token.as_deref(),
    )
    .await
    {
        Ok(mut paginated_files) => {
            if query.continuation_token.is_none() {
                paginated_files.total_count =
                    storage::rustfs::count_files(&s3_client, &collection.bucket)
                        .await
                        .ok();
            }
            HttpResponse::Ok().json(paginated_files)
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("error listing files: {e:?}")),
    }
}

#[utoipa::path(
    params(
        ("collection_id", description = "Collection ID"),
        ("file_key", description = "File key/name"),
    ),
    responses(
        (status = 200, description = "OK", content_type = "application/octet-stream"),
        (status = 404, description = "Collection or file not found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[get("/api/collections/{collection_id}/files/{file_key}")]
#[tracing::instrument(name = "download_collection_file", skip(auth, s3_client, postgres_pool, path), fields(collection_id = %path.0, file_key = %path.1))]
pub(crate) async fn download_collection_file(
    auth: Authenticated,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, String)>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let (collection_id, file_key) = path.into_inner();

    let collection =
        match collections::get_collection(&postgres_pool.into_inner(), &username, collection_id)
            .await
        {
            Ok(collection) => collection,
            Err(_) => {
                return HttpResponse::NotFound()
                    .body(format!("collection '{collection_id}' not found"));
            }
        };

    match storage::rustfs::get_file(&s3_client.into_inner(), &collection.bucket, &file_key).await {
        Ok(file_data) => {
            let mime_type = mime_guess::from_path(&file_key)
                .first_or_octet_stream()
                .to_string();

            HttpResponse::Ok()
                .content_type(mime_type)
                .insert_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", file_key),
                ))
                .body(file_data)
        }
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("error downloading file: {e:?}"))
        }
    }
}

#[utoipa::path(
    params(
        ("collection_id", description = "Collection ID"),
        ("file_key", description = "File key/name"),
    ),
    responses(
        (status = 200, description = "File deleted successfully"),
        (status = 404, description = "Collection or file not found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[delete("/api/collections/{collection_id}/files/{file_key}")]
#[tracing::instrument(name = "delete_collection_file", skip(auth, s3_client, postgres_pool, path), fields(collection_id = %path.0, file_key = %path.1))]
pub(crate) async fn delete_collection_file(
    auth: Authenticated,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, String)>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };
    let (collection_id, file_key) = path.into_inner();

    let collection =
        match collections::get_collection(&postgres_pool.into_inner(), &username, collection_id)
            .await
        {
            Ok(collection) => collection,
            Err(_) => {
                return HttpResponse::NotFound()
                    .body(format!("collection '{collection_id}' not found"));
            }
        };

    match delete_file(&s3_client.into_inner(), &collection.bucket, &file_key).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().body(format!("error deleting file: {e:?}")),
    }
}
