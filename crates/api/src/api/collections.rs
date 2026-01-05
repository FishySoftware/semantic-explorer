use actix_multipart::form::{MultipartForm, tempfile::TempFile};
use actix_web::{
    HttpResponse, Responder, delete, get, post,
    web::{self, Data, Json, Path},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use anyhow::Result;
use aws_sdk_s3::Client;
use sqlx::{Pool, Postgres};
use tokio::io::AsyncReadExt;
use tracing::error;
use uuid::Uuid;

use crate::{
    auth::extract_username,
    collections::models::{
        Collection, CollectionUpload, CollectionUploadResponse, CreateCollection, FileListQuery,
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
        Err(e) => return e,
    };
    let s3_client = s3_client.into_inner();
    let postgres_pool = postgres_pool.into_inner();
    let collection_id = collection_id.into_inner();

    let collection =
        match collections::get_collection(&postgres_pool, &username, collection_id).await {
            Ok(collection) => collection,
            Err(_) => {
                return HttpResponse::BadRequest()
                    .body(format!("collection '{collection_id}' does not exists"));
            }
        };

    let mut completed = Vec::with_capacity(payload.files.len());
    let mut failed = Vec::new();

    for file in payload.files {
        let file_name = file.file_name.clone().unwrap_or("unknown".to_string());
        let document = match to_uploaded_document(&file, collection.bucket.clone()).await {
            Ok(document) => document,
            Err(e) => {
                failed.push(file_name);
                error!("error processing file: {file:?} due to: {e:?}");
                continue;
            }
        };

        if let Err(e) = upload_document(&s3_client, document).await {
            failed.push(file_name);
            error!("error uploading file: {file:?} due to: {e:?}");
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

async fn to_uploaded_document(
    temp_file: &TempFile,
    collection_id: String,
) -> Result<DocumentUpload> {
    let name = match &temp_file.file_name {
        Some(name) => name.clone(),
        None => "unnamed".to_string(),
    };
    let mime_type = mime_guess::from_path(&name)
        .first_or_octet_stream()
        .to_string();

    let mut file = tokio::fs::File::open(temp_file.file.path()).await?;
    let mut content = Vec::with_capacity(temp_file.size);
    file.read_to_end(&mut content).await?;

    Ok(DocumentUpload {
        collection_id,
        name,
        content,
        mime_type,
    })
}
