use actix_web::{FromRequest, HttpRequest, HttpResponse, dev::Payload};
use actix_web_openidconnect::openid_middleware::Authenticated;
use futures_util::future::{Ready, err, ok};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use utoipa::ToSchema;

use crate::audit::events;
use crate::errors::{ApiError, unauthorized};

pub(crate) mod oidc;
pub(crate) mod session_manager;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct User {
    username: String,
    email: String,
    avatar: Option<String>,
}

/// Extractor for authenticated user information.
///
/// This extractor automatically extracts the username from the OIDC token,
/// eliminating the need for repetitive `extract_username` calls in handlers.
///
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub String);

impl Deref for AuthenticatedUser {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        // Extract client IP for audit logging
        let client_ip = req.connection_info().peer_addr().map(|s| s.to_string());

        // First, extract the Authenticated from the request
        match Authenticated::from_request(req, payload).into_inner() {
            Ok(auth) => {
                // Then extract the username from the token
                match auth.access.preferred_username() {
                    Some(username) => ok(AuthenticatedUser(username.to_string())),
                    None => {
                        events::auth_failed(
                            "unknown",
                            "user has no username in the user info claim",
                            client_ip.as_deref(),
                        );
                        err(ApiError::Unauthorized(
                            "user has no username in the user info claim".to_string(),
                        )
                        .into())
                    }
                }
            }
            Err(e) => {
                events::auth_failed(
                    "anonymous",
                    "authentication failed - invalid or missing token",
                    client_ip.as_deref(),
                );
                err(e)
            }
        }
    }
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
        None => Err(unauthorized("user has no username in the user info claim.")),
    }
}

pub(crate) fn extract_email(auth: &Authenticated) -> Result<String, HttpResponse> {
    match auth.access.email() {
        Some(email) => Ok(email.to_string()),
        None => Err(unauthorized("user has no email in the user info claim.")),
    }
}

pub(crate) fn extract_avatar(auth: &Authenticated) -> Option<String> {
    auth.access
        .picture()
        .and_then(|p| p.get(None))
        .map(|url| url.as_str().to_string())
}
