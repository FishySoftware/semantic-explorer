use std::sync::Arc;

use actix_web::{HttpResponse, error, web};
use semantic_explorer_core::http_client::HTTP_CLIENT;
use serde::{Deserialize, Serialize};
use tracing::warn;

use super::openid::OpenID;

#[derive(Serialize)]
struct DeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
pub(crate) struct DevicePollRequest {
    device_code: String,
}

#[derive(Serialize)]
struct DevicePollResponse {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    token_type: String,
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
pub(crate) struct RefreshRequest {
    refresh_token: String,
}

#[derive(Serialize)]
struct RefreshResponse {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    token_type: String,
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct DexDeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    #[serde(default = "default_expires_in")]
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_expires_in() -> u64 {
    300
}

fn default_interval() -> u64 {
    5
}

#[derive(Deserialize)]
struct DexTokenResponse {
    access_token: String,
    id_token: Option<String>,
    refresh_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct DexTokenErrorResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

fn encode_form_body(params: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params.iter().copied())
        .finish()
}

pub(crate) async fn device_authorize_endpoint(
    open_id_client: web::Data<Arc<OpenID>>,
) -> actix_web::Result<HttpResponse> {
    let device_endpoint = open_id_client
        .device_authorization_endpoint()
        .ok_or_else(|| {
            error::ErrorNotImplemented(
                "OIDC provider does not support the device authorization grant",
            )
        })?;

    let form_body = encode_form_body(&[
        ("client_id", open_id_client.client_id()),
        ("scope", "openid email profile offline_access"),
    ]);

    let response = HTTP_CLIENT
        .post(device_endpoint.as_str())
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form_body)
        .send()
        .await
        .map_err(|err| {
            warn!("Device authorization request failed: {err}");
            error::ErrorBadGateway("Failed to contact identity provider for device authorization")
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        warn!("Device authorization request returned {status}: {body}");
        return Err(error::ErrorBadGateway(
            "Identity provider rejected device authorization request",
        ));
    }

    let dex_response: DexDeviceAuthorizationResponse = response.json().await.map_err(|err| {
        warn!("Failed to parse device authorization response: {err}");
        error::ErrorBadGateway("Invalid response from identity provider")
    })?;

    Ok(HttpResponse::Ok().json(DeviceAuthorizationResponse {
        device_code: dex_response.device_code,
        user_code: dex_response.user_code,
        verification_uri: dex_response.verification_uri,
        verification_uri_complete: dex_response.verification_uri_complete,
        expires_in: dex_response.expires_in,
        interval: dex_response.interval,
    }))
}

pub(crate) async fn device_poll_endpoint(
    body: web::Json<DevicePollRequest>,
    open_id_client: web::Data<Arc<OpenID>>,
) -> actix_web::Result<HttpResponse> {
    let token_endpoint = open_id_client.token_endpoint().ok_or_else(|| {
        error::ErrorNotImplemented("OIDC provider does not expose a token endpoint")
    })?;

    let form_body = encode_form_body(&[
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("device_code", &body.device_code),
        ("client_id", open_id_client.client_id()),
    ]);

    let response = HTTP_CLIENT
        .post(token_endpoint.as_str())
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form_body)
        .send()
        .await
        .map_err(|err| {
            warn!("Device poll request failed: {err}");
            error::ErrorBadGateway("Failed to contact identity provider for device token poll")
        })?;

    let status = response.status();

    if status.is_success() {
        let token_response: DexTokenResponse = response.json().await.map_err(|err| {
            warn!("Failed to parse device poll token response: {err}");
            error::ErrorBadGateway("Invalid token response from identity provider")
        })?;

        return Ok(HttpResponse::Ok().json(DevicePollResponse {
            access_token: token_response.access_token,
            id_token: token_response.id_token,
            refresh_token: token_response.refresh_token,
            token_type: token_response
                .token_type
                .unwrap_or_else(|| "Bearer".to_string()),
            expires_in: token_response.expires_in,
        }));
    }

    let body_text = response.text().await.unwrap_or_default();

    if let Ok(error_response) = serde_json::from_str::<DexTokenErrorResponse>(&body_text) {
        return match error_response.error.as_str() {
            "authorization_pending" => Ok(HttpResponse::build(
                actix_web::http::StatusCode::PRECONDITION_REQUIRED,
            )
            .json(serde_json::json!({
                "error": "authorization_pending",
                "error_description": error_response.error_description
                    .unwrap_or_else(|| "User has not yet authorized the device".to_string()),
            }))),
            "slow_down" => Ok(HttpResponse::build(
                actix_web::http::StatusCode::PRECONDITION_REQUIRED,
            )
            .json(serde_json::json!({
                "error": "slow_down",
                "error_description": error_response.error_description
                    .unwrap_or_else(|| "Polling too frequently, please slow down".to_string()),
            }))),
            "expired_token" => Err(error::ErrorGone(
                error_response
                    .error_description
                    .unwrap_or_else(|| "Device code has expired".to_string()),
            )),
            "access_denied" => Err(error::ErrorForbidden(
                error_response
                    .error_description
                    .unwrap_or_else(|| "User denied the authorization request".to_string()),
            )),
            _ => {
                warn!(
                    "Device poll returned unexpected error: {} — {}",
                    error_response.error,
                    error_response.error_description.as_deref().unwrap_or("")
                );
                Err(error::ErrorBadGateway(format!(
                    "Identity provider error: {}",
                    error_response.error
                )))
            }
        };
    }

    warn!("Device poll returned {status} with unparseable body: {body_text}");
    Err(error::ErrorBadGateway(
        "Unexpected response from identity provider",
    ))
}

pub(crate) async fn refresh_token_endpoint(
    body: web::Json<RefreshRequest>,
    open_id_client: web::Data<Arc<OpenID>>,
) -> actix_web::Result<HttpResponse> {
    let token_endpoint = open_id_client.token_endpoint().ok_or_else(|| {
        error::ErrorNotImplemented("OIDC provider does not expose a token endpoint")
    })?;

    let form_body = encode_form_body(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &body.refresh_token),
        ("client_id", open_id_client.client_id()),
        ("scope", "openid email profile offline_access"),
    ]);

    let response = HTTP_CLIENT
        .post(token_endpoint.as_str())
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form_body)
        .send()
        .await
        .map_err(|err| {
            warn!("Refresh token request failed: {err}");
            error::ErrorBadGateway("Failed to contact identity provider for token refresh")
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();
        warn!("Refresh token request returned {status}: {body_text}");

        if let Ok(error_response) = serde_json::from_str::<DexTokenErrorResponse>(&body_text) {
            return match error_response.error.as_str() {
                "invalid_grant" => Err(error::ErrorUnauthorized(
                    error_response
                        .error_description
                        .unwrap_or_else(|| "Refresh token is invalid or expired".to_string()),
                )),
                _ => Err(error::ErrorBadGateway(format!(
                    "Identity provider error: {}",
                    error_response.error
                ))),
            };
        }

        return Err(error::ErrorBadGateway(
            "Identity provider rejected token refresh request",
        ));
    }

    let token_response: DexTokenResponse = response.json().await.map_err(|err| {
        warn!("Failed to parse refresh token response: {err}");
        error::ErrorBadGateway("Invalid token response from identity provider")
    })?;

    Ok(HttpResponse::Ok().json(RefreshResponse {
        access_token: token_response.access_token,
        id_token: token_response.id_token,
        refresh_token: token_response.refresh_token,
        token_type: token_response
            .token_type
            .unwrap_or_else(|| "Bearer".to_string()),
        expires_in: token_response.expires_in,
    }))
}
