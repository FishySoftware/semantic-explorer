use actix_web::{FromRequest, HttpRequest, HttpResponse, dev::Payload};
use futures_util::future::{Ready, err, ok};
use semantic_explorer_core::owner_info::OwnerInfo;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::Deref;
use utoipa::ToSchema;

use crate::audit::events;
use crate::errors::{ApiError, unauthorized};

pub(crate) mod device_flow;
pub(crate) mod oidc;
mod openid;
pub(crate) mod openid_middleware;

use openid_middleware::Authenticated;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct User {
    username: String,
    email: String,
    avatar: Option<String>,
}

/// Extractor for authenticated user information.
///
/// The `username` field holds the display name from the OIDC `preferred_username`
/// claim. The `sub` field holds the stable subject identifier from the `sub` claim,
/// which is used as the ownership key (`as_owner()`). Using `sub` avoids the
/// security issue where a reassigned/renamed `preferred_username` would grant the
/// new holder access to the previous owner's resources.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// Display name from OIDC `preferred_username` claim.
    pub username: String,
    /// Stable subject identifier from OIDC `sub` claim. Used to derive owner keys.
    sub: String,
}

impl AuthenticatedUser {
    /// Deterministic owner identifier derived from the stable `sub` claim.
    /// Use this for database owner fields, NATS subjects, and S3 paths.
    pub fn as_owner(&self) -> String {
        hash_sub_for_owner(&self.sub)
    }

    /// Convert to OwnerInfo struct for database operations.
    pub fn to_owner_info(&self) -> OwnerInfo {
        OwnerInfo::new(self.as_owner(), self.username.clone())
    }
}

impl Deref for AuthenticatedUser {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.username
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        match Authenticated::from_request(req, payload).into_inner() {
            Ok(auth) => {
                let username = match auth.access.preferred_username() {
                    Some(u) => u.to_string(),
                    None => {
                        events::auth_failed(
                            "unknown",
                            "unknown",
                            "user has no username in the user info claim",
                        );
                        return err(ApiError::Unauthorized(
                            "user has no username in the user info claim".to_string(),
                        )
                        .into());
                    }
                };
                // Use the stable `sub` claim as the owner key; fall back to
                // username only if the provider omits `sub` (non-compliant provider).
                let sub = auth.access.sub.clone().unwrap_or(username.clone());
                ok(AuthenticatedUser { username, sub })
            }
            Err(e) => {
                events::auth_failed(
                    "anonymous",
                    "anonymous",
                    "authentication failed - invalid or missing token",
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

/// Derive a safe, deterministic owner identifier from the OIDC `sub` claim.
///
/// `sub` is the stable, unique subject identifier guaranteed by the OIDC spec
/// (unlike `preferred_username`, which is mutable and provider-dependent).
/// The full 256-bit SHA-256 of `sub` is used here to avoid birthday-bound
/// collisions; the result is safe for use in:
/// - Database owner fields
/// - NATS subject hierarchies (which use `.` as delimiter)
/// - S3 object key prefixes
pub(crate) fn hash_sub_for_owner(sub: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sub.as_bytes());
    hex::encode(hasher.finalize())
}

pub(crate) fn extract_email(auth: &Authenticated) -> Result<String, HttpResponse> {
    match auth.access.email() {
        Some(email) => Ok(email.to_string()),
        None => Err(unauthorized("user has no email in the user info claim.")),
    }
}

pub(crate) fn extract_avatar(auth: &Authenticated) -> Option<String> {
    auth.access.picture_url().map(|url| url.to_string())
}
