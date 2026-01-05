use actix_web::dev::ServiceRequest;
use actix_web_openidconnect::ActixWebOpenId;
use anyhow::Result;

pub(crate) async fn initialize_client(redirect_url: String) -> Result<ActixWebOpenId> {
    let client_id = std::env::var("OIDC_CLIENT_ID")?;
    let client_secret = std::env::var("OIDC_CLIENT_SECRET")?;
    let issuer_url = std::env::var("OIDC_ISSUER_URL")?;
    let use_pkce = std::env::var("OIDC_USE_PKCE")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";
    let should_auth = |req: &ServiceRequest| {
        !req.path().eq_ignore_ascii_case("/metrics")
            && !req.path().eq_ignore_ascii_case("/health")
            && req.method() != actix_web::http::Method::OPTIONS
    };
    ActixWebOpenId::builder(client_id, redirect_url, issuer_url)
        .client_secret(client_secret)
        .scopes(vec![
            "openid".to_string(),
            "email".to_string(),
            "profile".to_string(),
        ])
        .use_pkce(use_pkce)
        .should_auth(should_auth)
        .build_and_init()
        .await
}
