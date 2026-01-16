use actix_web::{FromRequest, HttpRequest, HttpResponse, dev::Payload};
use actix_web_openidconnect::openid_middleware::Authenticated;
use futures_util::future::{Ready, err, ok};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::Deref;
use utoipa::ToSchema;

use crate::audit::events;
use crate::errors::{ApiError, unauthorized};

pub(crate) mod oidc;

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
/// The username is kept as-is from the OIDC token. When using the username
/// as an "owner" identifier for database records, NATS subjects, or S3 paths,
/// use `user.as_owner()` to get a hashed, infrastructure-safe version.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub String);

impl AuthenticatedUser {
    /// Get the hashed version of the username for use as an owner identifier.
    /// This should be used when the username is stored in database owner fields,
    /// used in NATS subjects, or used in S3 object paths.
    pub fn as_owner(&self) -> String {
        hash_username_for_owner(&self.0)
    }
}

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

/// Hash a username to create a safe, deterministic identifier for infrastructure use.
///
/// This is used to convert usernames (which may be email addresses with special
/// characters like @ and .) into URL-safe identifiers suitable for:
/// - NATS subject hierarchies (which use . as delimiter)
/// - S3 object keys
/// - Database owner fields
///
/// The hash is deterministic (same input = same output) and uses the first
/// 16 characters of the SHA256 hash in hexadecimal format.
///
/// The original username is preserved in authentication contexts, but this hashed
/// version should be used when the username is used as an "owner" identifier
/// in database records, NATS subjects, or S3 paths.
pub(crate) fn hash_username_for_owner(username: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    let result = hasher.finalize();
    // Use first 16 chars (64 bits) of hex - sufficient for uniqueness in this context
    format!("{:x}", result).chars().take(16).collect()
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
