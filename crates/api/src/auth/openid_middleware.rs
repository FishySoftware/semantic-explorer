use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::future::{Ready, ready};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::storage::valkey::{self, ValkeyClients};

use super::openid::{ExtendedIdToken, OpenID};
use actix_web::body::BoxBody;
use actix_web::cookie::time::Duration;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::dev::forward_ready;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::error::{self, ErrorUnauthorized};
use actix_web::http::StatusCode;
use actix_web::http::header::{AUTHORIZATION, HeaderValue, LOCATION};
use actix_web::{Error, FromRequest, HttpMessage, HttpRequest, HttpResponse, web};
use futures_util::future::LocalBoxFuture;
use openidconnect::core::CoreGenderClaim;
use openidconnect::{AccessToken, AuthorizationCode, EmptyAdditionalClaims, UserInfoClaims};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, error, warn};

/// Extract a Bearer token from the Authorization header, if present.
/// The "Bearer " prefix is optional — a raw token is also accepted.
fn extract_bearer_token(req: &ServiceRequest) -> Option<String> {
    let value = req.headers().get(AUTHORIZATION)?.to_str().ok()?;
    if value.len() > 7 && value[..7].eq_ignore_ascii_case("bearer ") {
        Some(value[7..].to_string())
    } else {
        Some(value.to_string())
    }
}

enum AuthCookies {
    AccessToken,
    IdToken,
    RefreshToken,
    UserInfo,
    PkceVerifier,
    Nonce,
}

impl Display for AuthCookies {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            AuthCookies::AccessToken => write!(f, "access_token"),
            AuthCookies::IdToken => write!(f, "id_token"),
            AuthCookies::RefreshToken => write!(f, "refresh_token"),
            AuthCookies::UserInfo => write!(f, "user_info"),
            AuthCookies::Nonce => write!(f, "nonce"),
            AuthCookies::PkceVerifier => write!(f, "pkce_verifier"),
        }
    }
}

/// Helper to build a cookie with consistent attributes for multi-replica safety.
///
/// All cookies MUST have `Path=/` so they are sent to any endpoint on the domain,
/// including the OIDC callback path. Without this, the browser defaults the cookie
/// path to the request URI directory, which causes cookies to be lost when the
/// callback path differs from the originating request path.
fn build_session_cookie(name: String, value: impl Into<String>) -> Cookie<'static> {
    Cookie::build(name, value.into())
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .finish()
}

/// Build a cookie that also needs the Secure flag (for tokens).
fn build_secure_cookie(name: String, value: impl Into<String>) -> Cookie<'static> {
    Cookie::build(name, value.into())
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(true)
        .finish()
}

/// Build a removal cookie that clears a previously set cookie.
fn build_removal_cookie(name: String) -> Cookie<'static> {
    Cookie::build(name, "")
        .path("/")
        .max_age(Duration::ZERO)
        .finish()
}

/// Cached user info that can be serialized to/from cookies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedUserInfo {
    sub: Option<String>,
    preferred_username: Option<String>,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    picture: Option<String>,
}

impl CachedUserInfo {
    pub fn preferred_username(&self) -> Option<&str> {
        self.preferred_username.as_deref()
    }

    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    pub fn picture_url(&self) -> Option<&str> {
        self.picture.as_deref()
    }
}

#[derive(Clone)]
pub struct AuthenticatedUser {
    pub access: CachedUserInfo,
}

/// Convert UserInfoClaims from the OIDC provider into a serializable CachedUserInfo.
fn cached_user_info_from_userinfo(
    info: &UserInfoClaims<EmptyAdditionalClaims, CoreGenderClaim>,
) -> CachedUserInfo {
    CachedUserInfo {
        sub: Some(info.subject().to_string()),
        preferred_username: info.preferred_username().map(|u| u.to_string()),
        email: info.email().map(|e| e.to_string()),
        email_verified: info.email_verified(),
        name: info.name().and_then(|n| n.get(None)).map(|n| n.to_string()),
        picture: info
            .picture()
            .and_then(|p| p.get(None))
            .map(|u| u.as_str().to_string()),
    }
}

#[derive(Debug)]
enum AuthError {
    NotAuthenticated {
        issuer_url: String,
        nonce: String,
        pkce_verifier: String,
    },
}

impl Display for AuthError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::NotAuthenticated { .. } => write!(f, "Not authenticated"),
        }
    }
}

impl error::ResponseError for AuthError {
    fn status_code(&self) -> StatusCode {
        match *self {
            AuthError::NotAuthenticated { .. } => StatusCode::FOUND,
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let mut resp = HttpResponse::build(self.status_code()).body(self.to_string());
        match self {
            AuthError::NotAuthenticated {
                issuer_url,
                nonce,
                pkce_verifier,
            } => {
                // FIX: Set Path=/ so the callback endpoint receives these cookies
                // regardless of what path triggered the authentication redirect.
                // Also set SameSite=Lax and HttpOnly for security.
                if let Err(e) = resp.add_cookie(&build_session_cookie(
                    AuthCookies::Nonce.to_string(),
                    nonce.as_str(),
                )) {
                    error!("Failed to set nonce cookie: {e}");
                    return HttpResponse::InternalServerError()
                        .body(format!("Failed to set auth cookie: {e}"));
                }
                if let Err(e) = resp.add_cookie(&build_session_cookie(
                    AuthCookies::PkceVerifier.to_string(),
                    pkce_verifier.as_str(),
                )) {
                    error!("Failed to set pkce_verifier cookie: {e}");
                    return HttpResponse::InternalServerError()
                        .body(format!("Failed to set auth cookie: {e}"));
                }
                match HeaderValue::from_str(issuer_url) {
                    Ok(location) => {
                        resp.headers_mut().insert(LOCATION, location);
                    }
                    Err(e) => {
                        error!("Invalid issuer URL for redirect: {e}");
                        return HttpResponse::InternalServerError()
                            .body(format!("Invalid issuer URL: {e}"));
                    }
                }
                resp
            }
        }
    }
}

/// TTL for cached bearer token → userinfo lookups (seconds).
/// Matches the Valkey L2 default so L1 ≤ L2.
const BEARER_CACHE_TTL_SECS: u64 = 3600;

/// Default Valkey TTL for shared bearer token cache (seconds).
/// Overridden by `ValkeyConfig::bearer_cache_ttl_secs` (env `VALKEY_BEARER_CACHE_TTL_SECS`).
const DEFAULT_VALKEY_BEARER_TTL_SECS: u64 = 3600;

/// L1 in-memory cache keyed by token hash → (CachedUserInfo, insert_time).
/// Avoids a DB round-trip on every request from the same replica.
/// Uses std::sync::RwLock (not tokio) so it can be held across .await-free
/// sections without pinning to a single executor thread.
type BearerTokenCache = Arc<RwLock<HashMap<String, (CachedUserInfo, Instant)>>>;

/// Hash a bearer token with SHA-256 so raw credentials are never persisted.
fn hash_bearer_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) struct OpenIdMiddleware<S> {
    openid_client: Arc<OpenID>,
    service: Rc<S>,
    should_auth: fn(&ServiceRequest) -> bool,
    use_pkce: bool,
    redirect_path: String,
    bearer_cache: BearerTokenCache,
}

impl<S, B> Service<ServiceRequest> for OpenIdMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let srv = self.service.clone();
        let client = self.openid_client.clone();
        let client2 = self.openid_client.clone();
        let should_auth = self.should_auth;
        let path = req.path().to_string();
        let path2 = req.path().to_string();
        let use_pkce = self.use_pkce;

        let redirect_to_auth = move || -> AuthError {
            let auth_url = client2.get_authorization_url(path.clone(), use_pkce);
            AuthError::NotAuthenticated {
                issuer_url: auth_url.url.to_string(),
                nonce: auth_url.nonce.secret().to_string(),
                pkce_verifier: auth_url.pkce_verifier.unwrap_or_default(),
            }
        };

        let bearer_token = extract_bearer_token(&req);
        let bearer_cache = self.bearer_cache.clone();

        let redirect_path = self.redirect_path.clone();
        Box::pin(async move {
            if path2.starts_with(&redirect_path) || !should_auth(&req) {
                return srv.call(req).await;
            }

            // Strategy 1: Bearer token in Authorization header (API clients)
            // Returns 401 on failure — API clients should never get a 302 redirect.
            if let Some(token) = bearer_token {
                let token_hash = hash_bearer_token(&token);

                // L1: check in-memory cache (sub-microsecond, same-replica hits)
                let cached = bearer_cache.read().ok().and_then(|guard| {
                    let (info, inserted_at) = guard.get(&token_hash)?;
                    if inserted_at.elapsed().as_secs() < BEARER_CACHE_TTL_SECS {
                        Some(info.clone())
                    } else {
                        None
                    }
                });

                let user_info = if let Some(info) = cached {
                    info
                } else {
                    // L2: check Valkey cache (shared across replicas)
                    let valkey_hit =
                        if let Some(valkey) = req.app_data::<web::Data<ValkeyClients>>() {
                            let cache_key = format!("bearer:{token_hash}");
                            valkey::cache_get::<CachedUserInfo>(&valkey.read, &cache_key).await
                        } else {
                            None
                        };

                    if let Some(info) = valkey_hit {
                        // Populate L1 from L2 hit
                        if let Ok(mut guard) = bearer_cache.write() {
                            guard.insert(token_hash, (info.clone(), Instant::now()));
                        }
                        info
                    } else {
                        // Cache miss: call OIDC userinfo endpoint
                        let info = client
                            .user_info(AccessToken::new(token.clone()))
                            .await
                            .map_err(|_| {
                                debug!("Bearer token invalid or expired");
                                ErrorUnauthorized("Invalid or expired bearer token")
                            })?;
                        let cached_info = cached_user_info_from_userinfo(&info);

                        // Write to L1
                        if let Ok(mut guard) = bearer_cache.write() {
                            guard.insert(token_hash.clone(), (cached_info.clone(), Instant::now()));
                        }
                        // Write to L2 Valkey (fire-and-forget; don't block the response)
                        if let Some(valkey) = req.app_data::<web::Data<ValkeyClients>>() {
                            let conn = valkey.write.clone();
                            let cache_key = format!("bearer:{token_hash}");
                            let info_clone = cached_info.clone();
                            // Use config TTL if ValkeyConfig is registered, otherwise fallback
                            let ttl = req
                                .app_data::<web::Data<semantic_explorer_core::config::ValkeyConfig>>()
                                .map(|c| c.bearer_cache_ttl_secs)
                                .unwrap_or(DEFAULT_VALKEY_BEARER_TTL_SECS);
                            actix_web::rt::spawn(async move {
                                valkey::cache_set(&conn, &cache_key, &info_clone, ttl).await;
                            });
                        }

                        cached_info
                    }
                };

                req.extensions_mut()
                    .insert(AuthenticatedUser { access: user_info });
                return srv.call(req).await;
            }

            // Strategy 2: Cookie-based auth (browser sessions)
            // Returns 302 redirect to OIDC provider on failure.
            match req.cookie(AuthCookies::AccessToken.to_string().as_str()) {
                None => return Err(redirect_to_auth().into()),
                Some(token) => {
                    let token_hash = hash_bearer_token(token.value());

                    // Use cached user_info from cookie instead of calling the
                    // OIDC provider's userinfo endpoint on every request.
                    let user_info = if let Some(user_info_cookie) =
                        req.cookie(AuthCookies::UserInfo.to_string().as_str())
                    {
                        match serde_json::from_str::<CachedUserInfo>(user_info_cookie.value()) {
                            Ok(cached_info) => cached_info,
                            Err(_) => {
                                debug!(
                                    "Cached user_info cookie invalid, calling userinfo endpoint"
                                );
                                match client
                                    .user_info(AccessToken::new(token.value().to_string()))
                                    .await
                                {
                                    Ok(info) => cached_user_info_from_userinfo(&info),
                                    Err(_) => {
                                        debug!("Token not active, redirecting to auth");
                                        return Err(redirect_to_auth().into());
                                    }
                                }
                            }
                        }
                    } else {
                        // No user_info cookie — try Valkey L2 before hitting OIDC
                        let valkey_hit =
                            if let Some(valkey) = req.app_data::<web::Data<ValkeyClients>>() {
                                let cache_key = format!("bearer:{token_hash}");
                                valkey::cache_get::<CachedUserInfo>(&valkey.read, &cache_key).await
                            } else {
                                None
                            };

                        if let Some(info) = valkey_hit {
                            info
                        } else {
                            match client
                                .user_info(AccessToken::new(token.value().to_string()))
                                .await
                            {
                                Ok(info) => {
                                    let cached_info = cached_user_info_from_userinfo(&info);
                                    // Write to Valkey L2 so other replicas / subsequent requests benefit
                                    if let Some(valkey) = req.app_data::<web::Data<ValkeyClients>>()
                                    {
                                        let conn = valkey.write.clone();
                                        let cache_key = format!("bearer:{token_hash}");
                                        let info_clone = cached_info.clone();
                                        let ttl =
                                            req.app_data::<web::Data<
                                                semantic_explorer_core::config::ValkeyConfig,
                                            >>()
                                            .map(|c| c.bearer_cache_ttl_secs)
                                            .unwrap_or(DEFAULT_VALKEY_BEARER_TTL_SECS);
                                        actix_web::rt::spawn(async move {
                                            valkey::cache_set(&conn, &cache_key, &info_clone, ttl)
                                                .await;
                                        });
                                    }
                                    cached_info
                                }
                                Err(_) => {
                                    debug!("Token not active, redirecting to auth");
                                    return Err(redirect_to_auth().into());
                                }
                            }
                        }
                    };

                    req.extensions_mut()
                        .insert(AuthenticatedUser { access: user_info });
                }
            }
            srv.call(req).await
        })
    }
}

pub(crate) struct AuthenticateMiddlewareFactory {
    client: Arc<OpenID>,
    should_auth: fn(&ServiceRequest) -> bool,
    use_pkce: bool,
    redirect_path: String,
    bearer_cache: BearerTokenCache,
}

impl AuthenticateMiddlewareFactory {
    pub(crate) fn new(
        client: Arc<OpenID>,
        should_auth: fn(&ServiceRequest) -> bool,
        use_pkce: bool,
        redirect_path: String,
        bearer_cache: BearerTokenCache,
    ) -> Self {
        AuthenticateMiddlewareFactory {
            client,
            should_auth,
            use_pkce,
            redirect_path,
            bearer_cache,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthenticateMiddlewareFactory
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = OpenIdMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(OpenIdMiddleware {
            openid_client: self.client.clone(),
            service: Rc::new(service),
            should_auth: self.should_auth,
            use_pkce: self.use_pkce,
            redirect_path: self.redirect_path.clone(),
            bearer_cache: self.bearer_cache.clone(),
        }))
    }
}

#[derive(Deserialize)]
pub(crate) struct AuthQuery {
    code: String,
    state: String,
}

pub(crate) async fn logout_endpoint(
    req: HttpRequest,
    open_id_client: web::Data<Arc<OpenID>>,
) -> actix_web::Result<HttpResponse> {
    let id_token = match req.cookie(AuthCookies::IdToken.to_string().as_str()) {
        None => {
            debug!("No id token, redirecting to auth");
            return Err(error::ErrorBadRequest("missing id token"));
        }
        Some(id) => id.value().to_string(),
    };
    let logout_uri = open_id_client
        .get_logout_uri(&ExtendedIdToken::from_str(id_token.as_str())?)
        .map_err(|e| {
            error!("Failed to build logout URI: {e}");
            error::ErrorInternalServerError(format!("Failed to build logout URI: {e}"))
        })?;
    let mut response = HttpResponse::Found();
    response.append_header((LOCATION, logout_uri.to_string()));

    // Clear all auth cookies on logout
    let mut resp = response.finish();
    let _ = resp.add_cookie(&build_removal_cookie(AuthCookies::AccessToken.to_string()));
    let _ = resp.add_cookie(&build_removal_cookie(AuthCookies::IdToken.to_string()));
    let _ = resp.add_cookie(&build_removal_cookie(AuthCookies::RefreshToken.to_string()));
    let _ = resp.add_cookie(&build_removal_cookie(AuthCookies::UserInfo.to_string()));
    let _ = resp.add_cookie(&build_removal_cookie(AuthCookies::Nonce.to_string()));
    let _ = resp.add_cookie(&build_removal_cookie(AuthCookies::PkceVerifier.to_string()));
    Ok(resp)
}

async fn execute_auth_endpoint(
    req: &HttpRequest,
    open_id_client: &Arc<OpenID>,
    query: &AuthQuery,
) -> actix_web::Result<HttpResponse> {
    let nonce = req
        .cookie(AuthCookies::Nonce.to_string().as_str())
        .ok_or_else(|| {
            debug!("No nonce cookie found — this typically happens when the cookie path didn't match the callback URL");
            error::ErrorBadRequest("No nonce")
        })?
        .value()
        .to_string();

    let pkce_verifier = if open_id_client.use_pkce {
        Some(
            req.cookie(AuthCookies::PkceVerifier.to_string().as_str())
                .ok_or_else(|| {
                    debug!("No pkce_verifier cookie found");
                    error::ErrorBadRequest("No pkce verifier")
                })?
                .value()
                .to_string(),
        )
    } else {
        None
    };

    let tkn = open_id_client
        .get_token(
            AuthorizationCode::new(query.code.to_string()),
            pkce_verifier,
        )
        .await
        .map_err(|err| {
            warn!("Error getting token: {err}");
            error::ErrorBadRequest("Error getting token")
        })?;

    let claims = if let Some(ref id_token) = tkn.id_token {
        Some(
            open_id_client
                .verify_id_token(id_token, nonce)
                .await
                .map_err(|err| {
                    warn!("Error verifying id token: {err}");
                    error::ErrorInternalServerError("invalid id token")
                })?,
        )
    } else {
        None
    };

    // Fetch user info from the OIDC provider to cache in cookie.
    // This is the ONLY time we call the userinfo endpoint — subsequent
    // requests will read from the cached cookie instead.
    let cached_user_info: Option<CachedUserInfo> =
        if let Ok(info) = open_id_client.user_info(tkn.access_token.clone()).await {
            Some(cached_user_info_from_userinfo(&info))
        } else {
            claims.map(|id_claims| CachedUserInfo {
                sub: Some(id_claims.subject().to_string()),
                preferred_username: id_claims.preferred_username().map(|u| u.to_string()),
                email: id_claims.email().map(|e| e.to_string()),
                email_verified: id_claims.email_verified(),
                name: id_claims
                    .name()
                    .and_then(|n| n.get(None))
                    .map(|n| n.to_string()),
                picture: id_claims
                    .picture()
                    .and_then(|p| p.get(None))
                    .map(|u| u.as_str().to_string()),
            })
        };

    let mut response = HttpResponse::Found();

    // FIX: All cookies now have Path=/ so they work across all endpoints.
    // Access token and ID token cookies are Secure + HttpOnly.
    response
        .append_header((LOCATION, query.state.to_string()))
        .cookie(build_secure_cookie(
            AuthCookies::AccessToken.to_string(),
            tkn.access_token.secret(),
        ));

    if let Some(ref id_token) = tkn.id_token {
        response.cookie(build_secure_cookie(
            AuthCookies::IdToken.to_string(),
            id_token.to_string(),
        ));
    }

    // Cache user info in cookie for subsequent requests
    if let Some(ref info) = cached_user_info
        && let Ok(json) = serde_json::to_string(info)
    {
        response.cookie(build_session_cookie(
            AuthCookies::UserInfo.to_string(),
            json,
        ));
    }

    // Clean up temporary auth cookies (nonce, pkce_verifier) — they're
    // single-use and should not persist after the callback completes.
    response.cookie(build_removal_cookie(AuthCookies::Nonce.to_string()));
    response.cookie(build_removal_cookie(AuthCookies::PkceVerifier.to_string()));

    Ok(if let Some(ref token) = tkn.refresh_token {
        response
            .cookie(build_secure_cookie(
                AuthCookies::RefreshToken.to_string(),
                token.secret(),
            ))
            .finish()
    } else {
        response.finish()
    })
}

pub(crate) async fn auth_endpoint(
    req: HttpRequest,
    open_id_client: web::Data<Arc<OpenID>>,
    query: web::Query<AuthQuery>,
) -> actix_web::Result<HttpResponse> {
    let res = execute_auth_endpoint(&req, &open_id_client, &query).await;
    if res.is_err() && open_id_client.redirect_on_error {
        let url = open_id_client.get_authorization_url("/".to_string(), open_id_client.use_pkce);

        // FIX: When redirecting on error, also set the new nonce/pkce cookies
        // so the retry auth flow has valid cookies.
        let mut response = HttpResponse::Found();
        response.append_header((LOCATION, url.url.to_string()));
        response.cookie(build_session_cookie(
            AuthCookies::Nonce.to_string(),
            url.nonce.secret().to_string(),
        ));
        response.cookie(build_session_cookie(
            AuthCookies::PkceVerifier.to_string(),
            url.pkce_verifier.unwrap_or_default(),
        ));
        Ok(response.finish())
    } else {
        res
    }
}

pub struct Authenticated(AuthenticatedUser);

/// JSON response for the token endpoint — returns tokens directly instead of
/// setting cookies, suitable for API clients / CLI tools / service accounts.
#[derive(Serialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub user_info: CachedUserInfo,
}

/// POST /api/token — Exchange an authorization code for Bearer tokens.
///
/// API clients complete the OIDC authorization code flow externally (e.g. via
/// browser redirect or device flow), then POST the authorization code here to
/// receive a JSON response with the access token they can use in subsequent
/// `Authorization: Bearer <token>` headers.
///
/// Request body (JSON):
/// ```json
/// {
///   "code": "<authorization_code>",
///   "nonce": "<nonce_from_auth_request>",
///   "pkce_verifier": "<optional_pkce_verifier>"
/// }
/// ```
pub(crate) async fn token_endpoint(
    body: web::Json<TokenRequest>,
    open_id_client: web::Data<Arc<OpenID>>,
) -> actix_web::Result<HttpResponse> {
    let pkce_verifier = if open_id_client.use_pkce {
        Some(body.pkce_verifier.clone().ok_or_else(|| {
            error::ErrorBadRequest("pkce_verifier is required when PKCE is enabled")
        })?)
    } else {
        None
    };

    let tkn = open_id_client
        .get_token(AuthorizationCode::new(body.code.clone()), pkce_verifier)
        .await
        .map_err(|err| {
            warn!("Token exchange failed: {err}");
            error::ErrorBadRequest("Failed to exchange authorization code for tokens")
        })?;

    // Verify ID token if present
    if let Some(ref id_token) = tkn.id_token {
        open_id_client
            .verify_id_token(id_token, body.nonce.clone())
            .await
            .map_err(|err| {
                warn!("ID token verification failed: {err}");
                error::ErrorUnauthorized("Invalid ID token")
            })?;
    }

    // Fetch user info for the response
    let user_info = open_id_client
        .user_info(tkn.access_token.clone())
        .await
        .map_err(|err| {
            warn!("Failed to fetch user info: {err}");
            error::ErrorInternalServerError("Failed to fetch user info")
        })?;

    Ok(HttpResponse::Ok().json(TokenResponse {
        access_token: tkn.access_token.secret().to_string(),
        id_token: tkn.id_token.map(|t| t.to_string()),
        refresh_token: tkn.refresh_token.map(|t| t.secret().to_string()),
        token_type: "Bearer".to_string(),
        user_info: cached_user_info_from_userinfo(&user_info),
    }))
}

#[derive(Deserialize)]
pub(crate) struct TokenRequest {
    code: String,
    nonce: String,
    pkce_verifier: Option<String>,
}

/// GET /api/auth/token — Returns the current session's access token for API use.
///
/// Browser-authenticated users can call this endpoint to retrieve their
/// access token (stored in an HttpOnly cookie) for use as a Bearer token
/// in programmatic API calls.
///
/// Response (JSON):
/// ```json
/// {
///   "access_token": "<token>",
///   "token_type": "Bearer"
/// }
/// ```
pub(crate) async fn get_cookie_token_endpoint(req: HttpRequest) -> actix_web::Result<HttpResponse> {
    let token = req
        .cookie(&AuthCookies::AccessToken.to_string())
        .ok_or_else(|| {
            error::ErrorUnauthorized("No access token cookie found. Please log in first.")
        })?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "access_token": token.value(),
        "token_type": "Bearer",
    })))
}

/// GET /api/auth/authorize — Returns the OIDC authorization URL for API clients.
///
/// API clients that need to initiate the OIDC flow can call this endpoint to
/// get the authorization URL, nonce, and PKCE verifier (if PKCE is enabled).
/// They should redirect the user to the returned URL, then exchange the
/// authorization code at POST /api/token.
///
/// Response (JSON):
/// ```json
/// {
///   "authorization_url": "https://idp.example.com/auth?...",
///   "nonce": "<nonce>",
///   "pkce_verifier": "<optional_pkce_verifier>"
/// }
/// ```
pub(crate) async fn authorize_endpoint(
    open_id_client: web::Data<Arc<OpenID>>,
) -> actix_web::Result<HttpResponse> {
    let auth_url = open_id_client.get_authorization_url("/".to_string(), open_id_client.use_pkce);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "authorization_url": auth_url.url.to_string(),
        "nonce": auth_url.nonce.secret().to_string(),
        "pkce_verifier": auth_url.pkce_verifier,
    })))
}

impl FromRequest for Authenticated {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let value = req.extensions().get::<AuthenticatedUser>().cloned();
        let result = value.ok_or(ErrorUnauthorized("Unauthorized")).map(Self);

        ready(result)
    }
}

impl std::ops::Deref for Authenticated {
    type Target = AuthenticatedUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
