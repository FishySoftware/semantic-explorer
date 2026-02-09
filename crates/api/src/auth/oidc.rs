use actix_web::dev::ServiceRequest;
use actix_web_openidconnect::ActixWebOpenId;
use anyhow::Result;
use semantic_explorer_core::config::OidcConfig;

pub(crate) async fn initialize_client(
    redirect_url: String,
    oidc_config: &OidcConfig,
) -> Result<ActixWebOpenId> {
    let should_auth = |req: &ServiceRequest| {
        let path = req.path();
        // Exclude health check endpoints and metrics from auth
        !path.eq_ignore_ascii_case("/metrics")
            && !path.starts_with("/health")
            && req.method() != actix_web::http::Method::OPTIONS
    };
    ActixWebOpenId::builder(
        oidc_config.client_id.clone(),
        redirect_url,
        oidc_config.issuer_url.clone(),
    )
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
