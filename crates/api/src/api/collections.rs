use actix_multipart::form::MultipartForm;
use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, delete, get, patch, post,
    web::{self, Data, Json, Path},
};
use aws_sdk_s3::{Client, primitives::ByteStream};
use sqlx::{Pool, Postgres};
use std::{collections::HashMap, time::Instant};
use tracing::{error, info, warn};

use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    collections::models::{
        Collection, CollectionSearchQuery, CollectionUpload, CollectionUploadResponse,
        CreateCollection, FailedUploadFile, FileListQuery, PaginatedCollections, UpdateCollection,
    },
    errors::ApiError,
    storage::{
        self,
        postgres::collections,
        s3::{
            delete_file,
            models::{DocumentUpload, PaginatedFiles},
            upload_document,
        },
    },
    validation::validate_upload_file,
};
use semantic_explorer_core::{config::S3Config, validation};

#[utoipa::path(
    params(
        ("limit" = Option<i64>, Query, description = "Number of items to return (default 20)"),
        ("offset" = Option<i64>, Query, description = "Number of items to skip (default 0)"),
        ("search" = Option<String>, Query, description = "Optional search term"),
    ),
    responses(
        (status = 200, description = "OK", body = PaginatedCollections),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[get("/api/collections")]
#[tracing::instrument(name = "get_collections", skip(user, pool, query))]
pub(crate) async fn get_collections(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    let pool = &pool.into_inner();

    let limit: i64 = query
        .get("limit")
        .and_then(|l: &String| l.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 1000);

    let offset: i64 = query
        .get("offset")
        .and_then(|o: &String| o.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);

    match collections::get_collections_paginated(pool, &user.as_owner(), limit, offset).await {
        Ok((collection_list, total_count)) => {
            let response = PaginatedCollections {
                collections: collection_list,
                total_count,
                limit,
                offset,
            };

            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch collections");
            ApiError::Internal(format!("Failed to fetch collections: {}", e)).error_response()
        }
    }
}

#[utoipa::path(
    params(
        ("collection_id" = i32, Path, description = "Collection ID"),
    ),
    responses(
        (status = 200, description = "OK", body = Collection),
        (status = 404, description = "Collection not found"),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Collections",
)]
#[get("/api/collections/{collection_id}")]
#[tracing::instrument(name = "get_collection", skip(user, pool))]
pub(crate) async fn get_collection(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    path: Path<i32>,
) -> impl Responder {
    let collection_id = path.into_inner();
    let pool = &pool.into_inner();

    match collections::get_collection(pool, &user.as_owner(), collection_id).await {
        Ok(collection) => HttpResponse::Ok().json(collection),
        Err(e) => {
            if e.to_string().contains("no rows") {
                ApiError::NotFound(format!("Collection {} not found", collection_id))
                    .error_response()
            } else {
                tracing::error!(error = %e, collection_id = %collection_id, "failed to fetch collection");
                ApiError::Internal(format!("Failed to fetch collection: {}", e)).error_response()
            }
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
#[tracing::instrument(name = "search_collections", skip(user, pool, query), fields(query_len = query.q.as_ref().map(|q| q.len()).unwrap_or(0), limit = %query.limit, offset = %query.offset))]
pub(crate) async fn search_collections(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    web::Query(query): web::Query<CollectionSearchQuery>,
) -> impl Responder {
    let pool = pool.into_inner();

    let result = if let Some(search_query) = &query.q {
        collections::search_collections(
            &pool,
            &user.as_owner(),
            search_query,
            query.limit,
            query.offset,
        )
        .await
    } else {
        collections::get_collections_paginated(&pool, &user.as_owner(), query.limit, query.offset)
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
#[tracing::instrument(name = "create_collection", skip(user, pool, s3_client, s3_config, create_collection, req), fields(collection_title = %create_collection.title))]
pub(crate) async fn create_collections(
    user: AuthenticatedUser,
    req: HttpRequest,
    pool: Data<Pool<Postgres>>,
    s3_client: Data<Client>,
    s3_config: Data<S3Config>,
    Json(create_collection): Json<CreateCollection>,
) -> impl Responder {
    let pool = pool.into_inner();
    let s3_client = s3_client.into_inner();
    let s3_config = s3_config.into_inner();

    if let Err(e) = validation::validate_title(&create_collection.title) {
        events::validation_failed(&user.as_owner(), &user, "title", &e.to_string());
        return ApiError::Validation(e).error_response();
    }
    if let Some(ref details) = create_collection.details
        && let Err(e) = validation::validate_description(details)
    {
        events::validation_failed(&user.as_owner(), &user, "details", &e.to_string());
        return ApiError::Validation(e).error_response();
    }
    if let Err(e) = validation::validate_tags(&create_collection.tags) {
        events::validation_failed(&user.as_owner(), &user, "tags", &e.to_string());
        return ApiError::Validation(e).error_response();
    }

    let owner = user.to_owner_info();
    let collection = match collections::create_collection(
        &pool,
        &create_collection.title,
        create_collection.details.as_deref(),
        &owner,
        &create_collection.tags,
        create_collection.is_public,
    )
    .await
    {
        Ok(collection) => collection,
        Err(e) => {
            return ApiError::Internal(format!("error creating collection due to: {:?}", e))
                .error_response();
        }
    };

    let folder_key = collection.s3_folder_key();
    if let Err(e) = s3_client
        .put_object()
        .bucket(&s3_config.bucket_name)
        .key(&folder_key)
        .body(ByteStream::from_static(b""))
        .send()
        .await
    {
        error!(
            "Failed to create collection folder marker in S3 ({}): {}",
            folder_key, e
        );
    }

    events::resource_created_with_request(
        &req,
        &user.as_owner(),
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
#[tracing::instrument(name = "delete_collection", skip(user, s3_client, s3_config, pool, req), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn delete_collections(
    user: AuthenticatedUser,
    req: HttpRequest,
    s3_client: Data<Client>,
    s3_config: Data<S3Config>,
    pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
) -> impl Responder {
    let pool = pool.into_inner();
    let s3_config = s3_config.into_inner();
    let collection_id = collection_id.into_inner();

    let _collection =
        match collections::get_collection(&pool, &user.as_owner(), collection_id).await {
            Ok(collection) => collection,
            Err(_) => {
                return ApiError::NotFound("Collection not found".to_string()).error_response();
            }
        };

    if let Err(e) =
        storage::s3::empty_collection(s3_client.as_ref(), &s3_config.bucket_name, collection_id)
            .await
    {
        warn!(
            "Error emptying collection '{}' due to: {:?}",
            collection_id, e
        );
        return ApiError::Internal(format!("Error emptying collection due to: {:?}", e))
            .error_response();
    }

    match collections::delete_collection(&pool, collection_id, &user.as_owner()).await {
        Ok(_) => {
            events::resource_deleted_with_request(
                &req,
                &user.as_owner(),
                &user,
                ResourceType::Collection,
                &collection_id.to_string(),
            );
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete collection");
            ApiError::Internal(format!("Error deleting collection due to: {:?}", e))
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
#[tracing::instrument(name = "update_collection", skip(user, pool, update_collection), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn update_collections(
    user: AuthenticatedUser,
    pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    Json(update_collection): Json<UpdateCollection>,
) -> impl Responder {
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

    let pool = pool.into_inner();
    let collection_id = collection_id.into_inner();

    match collections::update_collection(
        &pool,
        collection_id,
        &user.as_owner(),
        &update_collection.title,
        update_collection.details.as_deref(),
        &update_collection.tags,
        update_collection.is_public,
    )
    .await
    {
        Ok(collection) => {
            events::resource_updated(
                &user.as_owner(),
                &user,
                ResourceType::Collection,
                &collection_id.to_string(),
            );
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
#[tracing::instrument(name = "upload_to_collection", skip(user, s3_client, s3_config, pool, payload), fields(collection_id = %collection_id.as_ref(), file_count = payload.files.len()))]
pub(crate) async fn upload_to_collection(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    s3_config: Data<S3Config>,
    pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    MultipartForm(payload): MultipartForm<CollectionUpload>,
) -> impl Responder {
    let s3_client = s3_client.into_inner();
    let s3_config = s3_config.into_inner();
    let pool = pool.into_inner();
    let collection_id = collection_id.into_inner();

    let collection = match collections::get_collection(&pool, &user.as_owner(), collection_id).await
    {
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
    let mut failed: Vec<FailedUploadFile> = Vec::new();

    for (idx, temp_file) in payload.files.iter().enumerate() {
        let item_start = Instant::now();
        let file_name = temp_file
            .file_name
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("file_{}", idx));

        let file_path = temp_file.file.path().to_owned();
        let file_size = temp_file.size;

        let file_bytes = match tokio::fs::read(&file_path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::error!(file_name = %file_name, error = %e, "Failed to read temp file");
                let item_duration = item_start.elapsed().as_secs_f64();
                semantic_explorer_core::observability::record_document_upload(
                    "collection",
                    item_duration,
                    false,
                );
                failed.push(FailedUploadFile {
                    name: file_name,
                    error: format!("Failed to read file: {}", e),
                });
                continue;
            }
        };

        let validation_result = validate_upload_file(&file_bytes, &file_name).await;

        // Free memory immediately after validation
        drop(file_bytes);

        if !validation_result.is_valid {
            tracing::warn!(
                file_name = %file_name,
                validation_errors = ?validation_result.validation_errors,
                "File validation failed, rejecting upload"
            );
            let item_duration = item_start.elapsed().as_secs_f64();
            semantic_explorer_core::observability::record_document_upload(
                "collection",
                item_duration,
                false,
            );
            let validation_error = validation_result.validation_errors.join("; ");
            failed.push(FailedUploadFile {
                name: file_name.clone(),
                error: format!("Validation failed: {}", validation_error),
            });

            crate::audit::events::file_validation_failed(
                &user.as_owner(),
                &user,
                collection_id,
                &file_name,
                &validation_result.validation_errors.join("; "),
            );
            continue;
        }

        // Upload to S3 with retries for transient failures
        let retry_policy = semantic_explorer_core::retry::s3_retry_policy();
        let max_attempts = retry_policy.max_attempts + 1;
        let mut upload_result = Ok(());

        for attempt in 0..max_attempts {
            if attempt > 0 {
                let delay = retry_policy.delay_for_attempt(attempt);
                info!(
                    file_name = %file_name,
                    attempt = attempt + 1,
                    max_attempts = max_attempts,
                    delay_ms = delay.as_millis() as u64,
                    "Retrying S3 upload after transient failure"
                );
                tokio::time::sleep(delay).await;
            }

            // Re-create the stream from file path for each attempt
            let content_stream = match ByteStream::from_path(&file_path).await {
                Ok(stream) => stream,
                Err(e) => {
                    upload_result = Err(anyhow::anyhow!("Failed to read file: {}", e));
                    break; // Not retryable
                }
            };

            let document = DocumentUpload {
                collection_id: collection.collection_id.to_string(),
                name: file_name.clone(),
                content: content_stream,
                mime_type: validation_result
                    .mime_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                size: file_size as u64,
            };

            match upload_document(&s3_client, &s3_config.bucket_name, document).await {
                Ok(()) => {
                    upload_result = Ok(());
                    if attempt > 0 {
                        info!(
                            file_name = %file_name,
                            attempt = attempt + 1,
                            "S3 upload succeeded after retry"
                        );
                    }
                    break;
                }
                Err(e) => {
                    warn!(
                        file_name = %file_name,
                        attempt = attempt + 1,
                        max_attempts = max_attempts,
                        error = %e,
                        "S3 upload attempt failed"
                    );
                    upload_result = Err(e);
                }
            }
        }

        if let Err(e) = upload_result {
            let item_duration = item_start.elapsed().as_secs_f64();
            semantic_explorer_core::observability::record_document_upload(
                "collection",
                item_duration,
                false,
            );
            failed.push(FailedUploadFile {
                name: file_name.clone(),
                error: format!("Upload to storage failed: {}", e),
            });
            error!(
                file_name = %file_name,
                error = %e,
                "Failed to upload file to S3 after all retry attempts"
            );
            continue;
        }

        let item_duration = item_start.elapsed().as_secs_f64();
        semantic_explorer_core::observability::record_document_upload(
            "collection",
            item_duration,
            true,
        );

        tracing::info!(
            file_name = %file_name,
            "File uploaded successfully"
        );

        completed.push(file_name);
    }

    if !completed.is_empty()
        && let Err(e) = collections::increment_collection_file_count(
            &pool,
            collection_id,
            completed.len() as i64,
        )
        .await
    {
        tracing::error!(
            "Failed to increment file count for collection {}: {:?}",
            collection_id,
            e
        );
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
#[tracing::instrument(name = "list_collection_files", skip(user, s3_client, s3_config, pool, query), fields(collection_id = %collection_id.as_ref()))]
pub(crate) async fn list_collection_files(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    s3_config: Data<S3Config>,
    pool: Data<Pool<Postgres>>,
    collection_id: Path<i32>,
    web::Query(query): web::Query<FileListQuery>,
) -> impl Responder {
    let s3_client = s3_client.into_inner();
    let s3_config = s3_config.into_inner();
    let collection_id = collection_id.into_inner();

    let collection = match collections::get_collection(
        &pool.into_inner(),
        &user.as_owner(),
        collection_id,
    )
    .await
    {
        Ok(collection) => collection,
        Err(_) => {
            return ApiError::NotFound(format!("Collection '{}' not found", collection_id))
                .error_response();
        }
    };

    match storage::s3::list_files(
        &s3_client,
        &s3_config.bucket_name,
        &collection.collection_id.to_string(),
        query.page_size,
        query.continuation_token.as_deref(),
    )
    .await
    {
        Ok(s3_files) => {
            let mut total_count = None;
            if query.continuation_token.is_none() {
                total_count = storage::s3::count_collection_files(
                    &s3_client,
                    &s3_config.bucket_name,
                    collection.collection_id,
                )
                .await
                .ok();
            }

            let paginated_files = PaginatedFiles {
                files: s3_files.files,
                page: 0,
                page_size: query.page_size,
                has_more: s3_files.continuation_token.is_some(),
                continuation_token: s3_files.continuation_token,
                total_count,
            };

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
#[tracing::instrument(name = "download_collection_file", skip(user, s3_client, s3_config, pool, path), fields(collection_id = %path.0, file_key = %path.1))]
pub(crate) async fn download_collection_file(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    s3_config: Data<S3Config>,
    pool: Data<Pool<Postgres>>,
    path: Path<(i32, String)>,
) -> impl Responder {
    let s3_client = s3_client.into_inner();
    let s3_config = s3_config.into_inner();
    let (collection_id, file_key) = path.into_inner();

    let collection = match collections::get_collection(
        &pool.into_inner(),
        &user.as_owner(),
        collection_id,
    )
    .await
    {
        Ok(collection) => collection,
        Err(_) => {
            return ApiError::NotFound(format!("Collection '{}' not found", collection_id))
                .error_response();
        }
    };

    match storage::s3::get_file_with_size_check(
        &s3_client,
        &s3_config.bucket_name,
        &collection.collection_id.to_string(),
        &file_key,
        s3_config.max_download_size_bytes,
    )
    .await
    {
        Ok(file_data) => {
            // Audit log the file download
            crate::audit::events::file_downloaded(
                &user.as_owner(),
                &user,
                collection_id,
                &file_key,
            );

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
#[tracing::instrument(name = "delete_collection_file", skip(user, s3_client, s3_config, pool, path), fields(collection_id = %path.0, file_key = %path.1))]
pub(crate) async fn delete_collection_file(
    user: AuthenticatedUser,
    s3_client: Data<Client>,
    s3_config: Data<S3Config>,
    pool: Data<Pool<Postgres>>,
    path: Path<(i32, String)>,
) -> impl Responder {
    let (collection_id, file_key) = path.into_inner();
    let pool = pool.into_inner();
    let s3_client = s3_client.into_inner();
    let s3_config = s3_config.into_inner();

    let collection = match collections::get_collection(&pool, &user.as_owner(), collection_id).await
    {
        Ok(collection) => collection,
        Err(_) => {
            return ApiError::NotFound(format!("collection '{}' not found", collection_id))
                .error_response();
        }
    };

    match delete_file(
        &s3_client,
        &s3_config.bucket_name,
        &collection.collection_id.to_string(),
        &file_key,
    )
    .await
    {
        Ok(_) => {
            // Atomically decrement file count after successful delete
            if let Err(e) =
                collections::decrement_collection_file_count(&pool, collection_id, 1).await
            {
                tracing::error!(
                    "Failed to decrement file count for collection {}: {:?}",
                    collection_id,
                    e
                );
            }
            HttpResponse::Ok().finish()
        }
        Err(e) => ApiError::Internal(format!("error deleting file: {:?}", e)).error_response(),
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "List of allowed MIME types", body = Vec<String>),
    ),
    tag = "Collections",
)]
#[get("/api/collections-allowed-file-types")]
#[tracing::instrument(name = "get_allowed_file_types")]
pub(crate) async fn get_allowed_file_types() -> impl Responder {
    use crate::validation::get_allowed_mime_types;

    HttpResponse::Ok().json(get_allowed_mime_types())
}
