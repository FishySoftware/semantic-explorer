use std::sync::Arc;

use actix_web::dev::ServiceRequest;
use actix_web::web;
use actix_web::web::ServiceConfig;
use anyhow::Result;
use semantic_explorer_core::config::OidcConfig;
use url::Url;

use super::device_flow;
use super::openid::OpenID;
use super::openid_middleware;
use super::openid_middleware::BearerTokenCache;

#[derive(Clone)]
pub(crate) struct ActixWebOpenId {
    openid_client: Arc<OpenID>,
    should_auth: fn(&ServiceRequest) -> bool,
    use_pkce: bool,
    redirect_path: String,
    logout_path: String,
    bearer_cache: BearerTokenCache,
}

struct ActixWebOpenIdBuilder {
    client_id: String,
    client_secret: Option<String>,
    redirect_url: Url,
    logout_path: String,
    issuer_url: String,
    should_auth: fn(&ServiceRequest) -> bool,
    post_logout_redirect_url: Option<String>,
    scopes: Vec<String>,
    additional_audiences: Vec<String>,
    use_pkce: bool,
    redirect_on_error: bool,
    allow_all_audiences: bool,
}

impl ActixWebOpenIdBuilder {
    fn client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    fn should_auth(mut self, f: fn(&ServiceRequest) -> bool) -> Self {
        self.should_auth = f;
        self
    }

    fn scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    fn use_pkce(mut self, pkce: bool) -> Self {
        self.use_pkce = pkce;
        self
    }

    async fn build_and_init(self) -> Result<ActixWebOpenId> {
        Ok(ActixWebOpenId {
            openid_client: Arc::new(
                OpenID::init(
                    self.client_id,
                    self.client_secret,
                    self.redirect_url.clone(),
                    self.issuer_url,
                    self.post_logout_redirect_url,
                    self.scopes,
                    self.additional_audiences,
                    self.allow_all_audiences,
                    self.use_pkce,
                    self.redirect_on_error,
                )
                .await?,
            ),
            redirect_path: self.redirect_url.path().to_string(),
            should_auth: self.should_auth,
            use_pkce: self.use_pkce,
            logout_path: self.logout_path,
            bearer_cache: openid_middleware::new_bearer_token_cache(),
        })
    }
}

impl ActixWebOpenId {
    fn builder(
        client_id: String,
        redirect_url: String,
        issuer_url: String,
    ) -> Result<ActixWebOpenIdBuilder> {
        Ok(ActixWebOpenIdBuilder {
            client_id,
            client_secret: None,
            redirect_url: Url::parse(redirect_url.as_str())
                .map_err(|e| anyhow::anyhow!("Invalid redirect URL '{redirect_url}': {e}"))?,
            logout_path: "/logout".to_string(),
            issuer_url,
            should_auth: |_| true,
            post_logout_redirect_url: None,
            scopes: vec!["openid".into()],
            additional_audiences: vec![],
            use_pkce: false,
            redirect_on_error: false,
            allow_all_audiences: false,
        })
    }

    pub(crate) fn configure_open_id(&self) -> impl Fn(&mut ServiceConfig) + use<'_> {
        let client = self.openid_client.clone();
        move |cfg: &mut ServiceConfig| {
            cfg.service(
                web::resource(self.redirect_path.clone())
                    .route(web::get().to(openid_middleware::auth_endpoint)),
            )
            .service(
                web::resource(self.logout_path.clone())
                    .route(web::get().to(openid_middleware::logout_endpoint)),
            )
            // Bearer token API endpoints — allow API clients to authenticate
            // without cookies by exchanging authorization codes for tokens.
            .service(
                web::resource("/api/token")
                    .route(web::post().to(openid_middleware::token_endpoint)),
            )
            .service(
                web::resource("/api/auth/authorize")
                    .route(web::get().to(openid_middleware::authorize_endpoint)),
            )
            .service(
                web::resource("/api/auth/device")
                    .route(web::post().to(device_flow::device_authorize_endpoint)),
            )
            .service(
                web::resource("/api/auth/device/poll")
                    .route(web::post().to(device_flow::device_poll_endpoint)),
            )
            .service(
                web::resource("/api/auth/refresh")
                    .route(web::post().to(device_flow::refresh_token_endpoint)),
            )
            .app_data(web::Data::new(client.clone()));
        }
    }

    pub(crate) fn get_middleware(&self) -> openid_middleware::AuthenticateMiddlewareFactory {
        openid_middleware::AuthenticateMiddlewareFactory::new(
            self.openid_client.clone(),
            self.should_auth,
            self.use_pkce,
            self.redirect_path.clone(),
            self.bearer_cache.clone(),
        )
    }
}

pub(crate) async fn initialize_client(
    redirect_url: String,
    oidc_config: &OidcConfig,
) -> Result<ActixWebOpenId> {
    let should_auth = |req: &ServiceRequest| {
        let path = req.path();
        // Exclude health check endpoints, metrics, and auth token endpoints from auth
        !path.eq_ignore_ascii_case("/metrics")
            && !path.starts_with("/health")
            && !path.eq_ignore_ascii_case("/api/token")
            && !path.starts_with("/api/auth/")
            && req.method() != actix_web::http::Method::OPTIONS
    };
    ActixWebOpenId::builder(
        oidc_config.client_id.clone(),
        redirect_url,
        oidc_config.issuer_url.clone(),
    )?
    .client_secret(oidc_config.client_secret.clone())
    .scopes(vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ])
    .use_pkce(oidc_config.use_pkce)
    .should_auth(should_auth)
    .build_and_init()
    .await
}
