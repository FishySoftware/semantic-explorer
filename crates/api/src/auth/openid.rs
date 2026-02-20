use anyhow::Result;
use oauth2::basic::{BasicErrorResponseType, BasicRevocationErrorResponse};
use oauth2::{
    EndpointMaybeSet, EndpointNotSet, EndpointSet, PkceCodeChallenge, PkceCodeVerifier,
    StandardErrorResponse, StandardRevocableToken,
};
use openidconnect::core::{
    CoreAuthDisplay, CoreAuthPrompt, CoreAuthenticationFlow, CoreClaimName, CoreClaimType,
    CoreClient, CoreClientAuthMethod, CoreGenderClaim, CoreGrantType, CoreIdTokenClaims,
    CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreJweKeyManagementAlgorithm,
    CoreJwsSigningAlgorithm, CoreResponseMode, CoreResponseType, CoreSubjectIdentifierType,
    CoreTokenIntrospectionResponse, CoreTokenResponse,
};
use openidconnect::{
    AccessToken, AdditionalProviderMetadata, AuthorizationCode, ClaimsVerificationError, ClientId,
    ClientSecret, CsrfToken, EmptyAdditionalClaims, EndSessionUrl, IssuerUrl, LogoutRequest, Nonce,
    OAuth2TokenResponse, PostLogoutRedirectUrl, ProviderMetadata, RedirectUrl, RefreshToken, Scope,
    TokenResponse, UserInfoClaims,
};
use openidconnect::{Client, IdToken, reqwest};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use url::Url;

#[derive(Clone)]
pub(crate) struct OpenID {
    client: ExtendedClient,
    client_id: String,
    provider_metadata: ExtendedProviderMetadata,
    post_logout_redirect_url: Option<String>,
    scopes: Vec<Scope>,
    additional_audiences: Vec<String>,
    pub(crate) redirect_on_error: bool,
    allow_all_audiences: bool,
    pub(crate) use_pkce: bool,
}

pub(crate) struct OpenIDTokens {
    pub access_token: AccessToken,
    pub id_token: Option<ExtendedIdToken>,
    pub refresh_token: Option<RefreshToken>,
}

pub(crate) struct AuthorizationUrl {
    pub url: Url,
    pub nonce: Nonce,
    pub pkce_verifier: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AdditionalMetadata {
    end_session_endpoint: Option<EndSessionUrl>,
    device_authorization_endpoint: Option<url::Url>,
}

impl AdditionalProviderMetadata for AdditionalMetadata {}

pub(crate) type ExtendedProviderMetadata = ProviderMetadata<
    AdditionalMetadata,
    CoreAuthDisplay,
    CoreClientAuthMethod,
    CoreClaimName,
    CoreClaimType,
    CoreGrantType,
    CoreJweContentEncryptionAlgorithm,
    CoreJweKeyManagementAlgorithm,
    CoreJsonWebKey,
    CoreResponseMode,
    CoreResponseType,
    CoreSubjectIdentifierType,
>;

pub(crate) type ExtendedClient = Client<
    EmptyAdditionalClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<BasicErrorResponseType>,
    CoreTokenResponse,
    CoreTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub(crate) type ExtendedIdToken = IdToken<
    EmptyAdditionalClaims,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
>;

fn get_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {e}"))
}

impl OpenID {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn init(
        client_id: String,
        client_secret: Option<String>,
        redirect_uri: Url,
        issuer_url: String,
        post_logout_redirect_url: Option<String>,
        scopes: Vec<String>,
        additional_audiences: Vec<String>,
        allow_all_audiences: bool,
        use_pkce: bool,
        redirect_on_error: bool,
    ) -> Result<Self> {
        let http_client = get_http_client()?;
        let provider_metadata =
            ExtendedProviderMetadata::discover_async(IssuerUrl::new(issuer_url)?, &http_client)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to discover OpenID Provider: {e}"))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata.clone(),
            ClientId::new(client_id.to_string()),
            client_secret.map(|client_secret| ClientSecret::new(client_secret.to_string())),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_uri.to_string())
                .map_err(|e| anyhow::anyhow!("Invalid redirect URL: {e}"))?,
        );

        Ok(Self {
            client,
            client_id,
            provider_metadata,
            post_logout_redirect_url,
            scopes: scopes.iter().map(|s| Scope::new(s.to_string())).collect(),
            additional_audiences,
            use_pkce,
            redirect_on_error,
            allow_all_audiences,
        })
    }

    pub(crate) fn get_authorization_url(&self, path: String, with_pkce: bool) -> AuthorizationUrl {
        let builder = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                move || CsrfToken::new(path.clone()),
                Nonce::new_random,
            )
            .add_scopes(self.scopes.clone());
        if with_pkce {
            let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
            let (url, .., nonce) = builder.set_pkce_challenge(pkce_challenge).url();

            AuthorizationUrl {
                url,
                nonce,
                pkce_verifier: Some(pkce_verifier.secret().clone()),
            }
        } else {
            let (url, .., nonce) = builder.url();
            AuthorizationUrl {
                url,
                nonce,
                pkce_verifier: None,
            }
        }
    }

    pub(crate) async fn get_token(
        &self,
        authorization_code: AuthorizationCode,
        pkce_verifier: Option<String>,
    ) -> Result<OpenIDTokens> {
        let http_client = get_http_client()?;
        let token_response = if let Some(pkce_verifier) = pkce_verifier {
            self.client
                .exchange_code(authorization_code)?
                .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        } else {
            self.client.exchange_code(authorization_code)?
        }
        .request_async(&http_client)
        .await?;

        let id_token = token_response.id_token().cloned();

        Ok(OpenIDTokens {
            access_token: token_response.access_token().clone(),
            id_token,
            refresh_token: token_response.refresh_token().cloned(),
        })
    }

    pub(crate) async fn user_info(
        &self,
        access_token: AccessToken,
    ) -> Result<UserInfoClaims<EmptyAdditionalClaims, CoreGenderClaim>> {
        let http_client = get_http_client()?;
        Ok(self
            .client
            .user_info(access_token, None)?
            .request_async(&http_client)
            .await?)
    }

    pub(crate) async fn verify_id_token<'a>(
        &self,
        id_token: &'a ExtendedIdToken,
        nonce: String,
    ) -> Result<&'a CoreIdTokenClaims, ClaimsVerificationError> {
        id_token.claims(
            &self
                .client
                .id_token_verifier()
                .set_other_audience_verifier_fn(|audience| {
                    self.allow_all_audiences || self.additional_audiences.contains(audience)
                }),
            &Nonce::new(nonce),
        )
    }

    pub(crate) fn get_logout_uri(&self, id_token: &ExtendedIdToken) -> Result<Url> {
        let end_session_endpoint = self
            .provider_metadata
            .additional_metadata()
            .end_session_endpoint
            .clone()
            .ok_or_else(|| {
                anyhow::anyhow!("OIDC provider does not expose an end_session_endpoint")
            })?;

        let mut logout_request =
            LogoutRequest::from(end_session_endpoint).set_id_token_hint(id_token);

        if let Some(ref uri) = self.post_logout_redirect_url {
            logout_request = logout_request.set_post_logout_redirect_uri(
                PostLogoutRedirectUrl::new(uri.to_string())
                    .map_err(|e| anyhow::anyhow!("Invalid post-logout redirect URL: {e}"))?,
            );
        }

        Ok(logout_request.http_get_url())
    }

    /// Return the device_authorization_endpoint from provider metadata, if Dex has it enabled.
    pub(crate) fn device_authorization_endpoint(&self) -> Option<&url::Url> {
        self.provider_metadata
            .additional_metadata()
            .device_authorization_endpoint
            .as_ref()
    }

    /// Return the token endpoint from provider metadata.
    pub(crate) fn token_endpoint(&self) -> Option<String> {
        self.provider_metadata
            .token_endpoint()
            .map(|url| url.to_string())
    }

    /// Return the client_id used by this OpenID client.
    pub(crate) fn client_id(&self) -> &str {
        // The client_id is set during init; retrieve it from the metadata.
        // CoreClient doesn't expose client_id directly, so we store it separately.
        &self.client_id
    }
}
