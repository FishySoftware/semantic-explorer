use actix_multipart::form::MultipartForm;
use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{self, Data, Json, Path},
};
use aws_sdk_s3::Client;
use futures_util::future::join_all;
use sqlx::{Pool, Postgres};
use tracing::error;
use uuid::Uuid;

use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    collections::models::{
        Collection, CollectionSearchQuery, CollectionUpload, CollectionUploadResponse,
        CreateCollection, FileListQuery, PaginatedCollections, UpdateCollection,
    },
    errors::ApiError,
    storage::{
        self,
        postgres::collections,
        rustfs::{
            delete_file, empty_bucket,
            models::{DocumentUpload, PaginatedFiles},
            upload_document,
        },
    },
};
use semantic_explorer_core::validation;

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = Vec<Collection>),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[get("/api/collections")]
#[tracing::instrument(name = "get_collections", skip(user, s3_client, postgres_pool))]
pub(crate) async fn get_collections(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match collections::get_collections(&postgres_pool.into_inner(), &user).await {
        Ok(mut collection_list) => {
            let s3_client = s3_client.as_ref();

            // Fetch file counts in parallel for all collections
            let count_futures: Vec<_> = collection_list
                .iter()
                .map(|c| storage::rustfs::count_files(s3_client, &c.bucket))
                .collect();

            let counts = join_all(count_futures).await;

            for (collection, count_result) in collection_list.iter_mut().zip(counts) {
                collection.file_count = count_result.ok();
            }

            HttpResponse::Ok().json(collection_list)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch collections");
            ApiError::Internal(format!("Failed to fetch collections: {}", e)).error_response()
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
#[tracing::instrument(name = "search_collections", skip(user, postgres_pool), fields(query = ?query.q, limit = %query.limit, offset = %query.offset))]
pub(crate) async fn search_collections(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    web::Query(query): web::Query<CollectionSearchQuery>,
) -> impl Responder {
    let postgres_pool = postgres_pool.into_inner();

    let result = if let Some(search_query) = &query.q {
        collections::search_collections(
            &postgres_pool,
            &user,
            search_query,
            query.limit,
            query.offset,
        )
        .await
    } else {
        collections::get_collections_paginated(&postgres_pool, &user, query.limit, query.offset)
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
            ApiError::Internal(format!("Failed to search collections: {}", e)).error_response()
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
#[tracing::instrument(name = "create_collection", skip(user, s3_client, postgres_pool, create_collection, req), fields(collection_title = %create_collection.title))]
pub(crate) async fn create_collections(
    user: AuthenticatedUser,
    req: HttpRequest,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    Json(create_collection): Json<CreateCollection>,
) -> impl Responder {
    // Input validation
    if let Err(e) = validation::validate_title(&create_collection.title) {
        events::validation_failed(&user, "title", &e.to_string());
        return ApiError::Validation(e).error_response();
    }
    if let Some(ref details) = create_collection.details
        && let Err(e) = validation::validate_description(details)
    {
        events::validation_failed(&user, "details", &e.to_string());
        return ApiError::Validation(e).error_response();
    }
    if let Err(e) = validation::validate_tags(&create_collection.tags) {
        events::validation_failed(&user, "tags", &e.to_string());
        return ApiError::Validation(e).error_response();
    }

    let s3_client = s3_client.into_inner();
    let bucket = Uuid::new_v4().to_string();

    if let Err(e) = s3_client.create_bucket().bucket(&bucket).send().await {
        return ApiError::Internal(format!(
            "error creating collection bucket '{}' due to: {:?}",
            bucket, e
        ))
        .error_response();
    }

    let collection = match collections::create_collection(
        &postgres_pool.into_inner(),
        &create_collection.title,
        create_collection.details.as_deref(),
        &user,
        &bucket,
        &create_collection.tags,
        create_collection.is_public,
    )
    .await
    {
        Ok(collection) => collection,
        Err(e) => {
            if let Err(del_err) = s3_client.delete_bucket().bucket(&bucket).send().await {
                error!(
                    "error deleting collection bucket '{}' due to: {:?}",
                    bucket, del_err
                );
            }
            return ApiError::Internal(format!("error creating collection due to: {:?}", e))
                .error_response();
        }
    };

    events::resource_created_with_request(
        &req,
        &user,
        ResourceType::Collection,
        &collection.collection_id.to_string(),
    );
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
#[tracing::instrument(name = "delete_collection", skip(user, s3_client, postgres_pool, req), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn delete_collections(
    user: AuthenticatedUser,
    req: HttpRequest,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
) -> impl Responder {
    let postgres_pool = postgres_pool.into_inner();
    let collection_id = collection_id.into_inner();

    let collection = match collections::get_collection(&postgres_pool, &user, collection_id).await {
        Ok(collection) => collection,
        Err(_) => {
            return ApiError::NotFound("Collection not found".to_string()).error_response();
        }
    };

    if let Err(e) = empty_bucket(s3_client.as_ref(), &collection.bucket).await {
        error!(
            "error emptying collection bucket '{}' due to: {:?}",
            collection_id, e
        );
        return ApiError::Internal(format!("error emptying collection bucket due to: {:?}", e))
            .error_response();
    }

    if let Err(e) = s3_client
        .into_inner()
        .delete_bucket()
        .bucket(&collection.bucket)
        .send()
        .await
    {
        error!("error deleting collection bucket '{collection_id}' due to: {e:?}");
    }

    match collections::delete_collection(&postgres_pool, collection_id, &user).await {
        Ok(_) => {
            events::resource_deleted_with_request(
                &req,
                &user,
                ResourceType::Collection,
                &collection_id.to_string(),
            );
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to delete collection");
            ApiError::Internal(format!("error deleting collection due to: {:?}", e))
                .error_response()
        }
    }
}

#[utoipa::path(
    request_body = UpdateCollection,
    responses(
        (status = 200, description = "Updated collection", body = Collection),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[patch("/api/collections/{collection_id}")]
#[tracing::instrument(name = "update_collection", skip(user, postgres_pool, update_collection), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn update_collections(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    Json(update_collection): Json<UpdateCollection>,
) -> impl Responder {
    // Validate input
    if let Err(e) = validation::validate_title(&update_collection.title) {
        return ApiError::Validation(e).error_response();
    }
    if let Some(details) = &update_collection.details
        && let Err(e) = validation::validate_description(details)
    {
        return ApiError::Validation(e).error_response();
    }
    if let Err(e) = validation::validate_tags(&update_collection.tags) {
        return ApiError::Validation(e).error_response();
    }

    let postgres_pool = postgres_pool.into_inner();
    let collection_id = collection_id.into_inner();

    match collections::update_collection(
        &postgres_pool,
        collection_id,
        &user,
        &update_collection.title,
        update_collection.details.as_deref(),
        &update_collection.tags,
        update_collection.is_public,
    )
    .await
    {
        Ok(collection) => {
            events::resource_updated(&user, ResourceType::Collection, &collection_id.to_string());
            HttpResponse::Ok().json(collection)
        }
        Err(_) => {
            return ApiError::NotFound("Collection not found".to_string()).error_response();
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
#[tracing::instrument(name = "upload_to_collection", skip(user, s3_client, postgres_pool, payload), fields(collection_id = %collection_id.as_ref(), file_count = payload.files.len()))]
pub(crate) async fn upload_to_collection(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    MultipartForm(payload): MultipartForm<CollectionUpload>,
) -> impl Responder {
    let s3_client = s3_client.into_inner();
    let postgres_pool = postgres_pool.into_inner();
    let collection_id = collection_id.into_inner();

    let collection = match collections::get_collection(&postgres_pool, &user, collection_id).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!(
                collection_id = collection_id,
                username = %*user,
                error = %e,
                "Collection not found or access denied"
            );
            return ApiError::BadRequest(format!("collection '{}' does not exist", collection_id))
                .error_response();
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
        let claimed_mime = mime_guess::from_path(&file_name)
            .first_or_octet_stream()
            .to_string();

        // Validate file before attempting upload (runs on blocking thread pool)
        let validation_result =
            crate::validation::validate_upload_file(&file_bytes.data, &file_name, &claimed_mime)
                .await;

        if !validation_result.is_valid {
            tracing::warn!(
                file_name = %file_name,
                validation_errors = ?validation_result.validation_errors,
                "File validation failed, rejecting upload"
            );
            failed.push(file_name.clone());

            // Audit log the rejected upload
            crate::audit::events::file_validation_failed(
                &user.0,
                collection_id,
                &file_name,
                &validation_result.validation_errors.join("; "),
            );
            continue;
        }

        let document = DocumentUpload {
            collection_id: collection.bucket.clone(),
            name: file_name.clone(),
            content: file_bytes.data.to_vec(),
            mime_type: validation_result.detected_mime.clone(),
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

        tracing::info!(
            file_name = %file_name,
            detected_mime = %validation_result.detected_mime,
            "File uploaded successfully"
        );
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
#[tracing::instrument(name = "list_collection_files", skip(user, s3_client, postgres_pool, query), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn list_collection_files(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    web::Query(query): web::Query<FileListQuery>,
) -> impl Responder {
    let collection_id = collection_id.into_inner();
    let collection = match collections::get_collection(
        &postgres_pool.into_inner(),
        &user,
        collection_id,
    )
    .await
    {
        Ok(collection) => collection,
        Err(_) => {
            return ApiError::NotFound(format!("collection '{}' not found", collection_id))
                .error_response();
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
        Err(e) => ApiError::Internal(format!("error listing files: {:?}", e)).error_response(),
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
#[tracing::instrument(name = "download_collection_file", skip(user, s3_client, postgres_pool, path, req), fields(collection_id = %path.0, file_key = %path.1))]
pub(crate) async fn download_collection_file(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, String)>,
    req: actix_web::HttpRequest,
) -> impl Responder {
    let (collection_id, file_key) = path.into_inner();

    let collection = match collections::get_collection(
        &postgres_pool.into_inner(),
        &user,
        collection_id,
    )
    .await
    {
        Ok(collection) => collection,
        Err(_) => {
            return ApiError::NotFound(format!("collection '{}' not found", collection_id))
                .error_response();
        }
    };

    match storage::rustfs::get_file_with_size_check(
        &s3_client.into_inner(),
        &collection.bucket,
        &file_key,
    )
    .await
    {
        Ok(file_data) => {
            // Audit log the file download
            crate::audit::events::file_downloaded(&req, &user.0, collection_id, &file_key);

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
            let error_msg = e.to_string();
            if error_msg.contains("exceeds maximum download limit") {
                ApiError::BadRequest(error_msg).error_response()
            } else {
                ApiError::Internal(format!("error downloading file: {:?}", e)).error_response()
            }
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
#[tracing::instrument(name = "delete_collection_file", skip(user, s3_client, postgres_pool, path), fields(collection_id = %path.0, file_key = %path.1))]
pub(crate) async fn delete_collection_file(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    postgres_pool: Data<Pool<Postgres>>,
    path: Path<(i32, String)>,
) -> impl Responder {
    let (collection_id, file_key) = path.into_inner();

    let collection = match collections::get_collection(
        &postgres_pool.into_inner(),
        &user,
        collection_id,
    )
    .await
    {
        Ok(collection) => collection,
        Err(_) => {
            return ApiError::NotFound(format!("collection '{}' not found", collection_id))
                .error_response();
        }
    };

    match delete_file(&s3_client.into_inner(), &collection.bucket, &file_key).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => ApiError::Internal(format!("error deleting file: {:?}", e)).error_response(),
    }
}
