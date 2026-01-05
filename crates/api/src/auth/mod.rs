use actix_web::HttpResponse;
use actix_web_openidconnect::openid_middleware::Authenticated;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub(crate) mod oidc;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct User {
    username: String,
    email: String,
    avatar: Option<String>,
}

pub(crate) fn extract_user(auth: &Authenticated) -> Result<User, HttpResponse> {
    let username = extract_username(auth)?;
    let email = extract_email(auth)?;
    let avatar = extract_avatar(auth);
    Ok(User {
        username,
        email,
        avatar,
    })
}

pub(crate) fn extract_username(auth: &Authenticated) -> Result<String, HttpResponse> {
    match auth.access.preferred_username() {
        Some(user) => Ok(user.to_string()),
        None => {
            Err(HttpResponse::Unauthorized().body("user has no username in the user info claim."))
        }
    }
}

pub(crate) fn extract_email(auth: &Authenticated) -> Result<String, HttpResponse> {
    match auth.access.email() {
        Some(email) => Ok(email.to_string()),
        None => Err(HttpResponse::Unauthorized().body("user has no email in the user info claim.")),
    }
}

pub(crate) fn extract_avatar(auth: &Authenticated) -> Option<String> {
    auth.access
        .picture()
        .and_then(|p| p.get(None))
        .map(|url| url.as_str().to_string())
}
