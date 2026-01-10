pub(crate) mod chat;
pub(crate) mod collection_transforms;
pub(crate) mod collections;
pub(crate) mod dataset_transforms;
pub(crate) mod datasets;
pub(crate) mod embedded_datasets;
pub(crate) mod embedders;
pub(crate) mod llms;
pub(crate) mod marketplace;
pub(crate) mod search;
pub(crate) mod visualization_transforms;

use crate::auth::extract_user;
use actix_files::NamedFile;
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    error::ErrorNotFound,
    get,
    web::{Data, redirect},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use std::path::PathBuf;
use tracing::error;

#[get("/api/users/@me")]
async fn get_current_user(auth: Authenticated) -> impl Responder {
    match extract_user(&auth) {
        Ok(user) => HttpResponse::Ok().json(user),
        Err(e) => e,
    }
}

#[get("/health")]
pub(crate) async fn health() -> impl Responder {
    HttpResponse::Ok().body("up".to_string())
}

#[get("/")]
pub(crate) async fn base(request: HttpRequest) -> impl Responder {
    let from = request.uri().to_string();
    let mut to = from.clone();
    if to.ends_with('/') {
        to.push_str("ui");
    } else {
        to.push_str("/ui");
    }
    redirect(from, to)
}

#[get("/ui")]
pub(crate) async fn index(
    request: HttpRequest,
    static_files_directory: Data<PathBuf>,
) -> impl Responder {
    get_named_file_response(&request, static_files_directory.join("index.html"))
}

#[get("/ui/{tail:.*}")]
pub(crate) async fn pages(
    request: HttpRequest,
    static_files_directory: Data<PathBuf>,
) -> impl Responder {
    match request.match_info().query("tail").parse::<PathBuf>() {
        Ok(path) => {
            let file_path = static_files_directory.join(path);
            if file_path.exists() && file_path.is_file() {
                return get_named_file_response(&request, file_path);
            }
            get_named_file_response(&request, static_files_directory.join("index.html"))
        }
        Err(e) => {
            error!("invalid static file requested: {e:?}");
            ErrorNotFound("not found.".to_string()).error_response()
        }
    }
}

fn get_named_file_response(request: &HttpRequest, file_path: PathBuf) -> HttpResponse {
    match NamedFile::open(file_path.clone()) {
        Ok(named_file) => named_file
            .use_last_modified(true)
            .use_etag(true)
            .into_response(request),
        Err(e) => ErrorNotFound(format!(
            "static file with path: '{file_path:?} was not found: {e:?}"
        ))
        .error_response(),
    }
}
