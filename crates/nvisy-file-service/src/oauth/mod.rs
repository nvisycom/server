//! OAuth2 authorization-code flow, driven through the shared `reqwest` client.
//!
//! Provides the reusable pieces every OAuth file-service provider needs: the
//! per-provider endpoint/scope description ([`OAuthProvider`]), the persisted
//! token set ([`OAuthTokens`]), building an authorize URL with PKCE and CSRF
//! state ([`begin_authorization`]), exchanging an authorization code
//! ([`exchange_code`]), and refreshing an access token ([`refresh_tokens`]).
//!
//! The `oauth2` crate is used with no built-in HTTP client; every token request
//! runs through [`reqwest_client`], an adapter over a caller-supplied
//! [`reqwest::Client`], so OAuth traffic shares the same TLS/proxy/tracing setup
//! as the rest of the platform.

mod http_client;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};

pub use self::http_client::reqwest_client;
use crate::error::{Error, ErrorKind};

/// The OAuth2 endpoints and scopes for one provider (e.g. Google).
#[derive(Debug, Clone)]
pub struct OAuthProvider {
    /// Authorization endpoint (where the user is sent to grant access).
    pub auth_url: String,
    /// Token endpoint (where codes and refresh tokens are exchanged).
    pub token_url: String,
    /// Scopes to request. Some providers (e.g. Box) configure scopes on the app
    /// rather than in the authorize URL, and leave this empty.
    pub scopes: Vec<String>,
    /// Extra provider-specific authorize-URL parameters needed to obtain a
    /// refresh token (e.g. Google's `access_type=offline`, Dropbox's
    /// `token_access_type=offline`).
    pub extra_authorize_params: Vec<(String, String)>,
}

/// The OAuth application's credentials and redirect, supplied by configuration.
#[derive(Debug, Clone)]
pub struct OAuthApp {
    /// The OAuth client id.
    pub client_id: String,
    /// The OAuth client secret.
    pub client_secret: String,
    /// The redirect URI registered with the provider; the callback route.
    pub redirect_uri: String,
}

/// A persisted OAuth token set. Stored encrypted with the rest of a
/// connection's config; never returned in API responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct OAuthTokens {
    /// The current access token, sent as a bearer credential.
    pub access_token: String,
    /// The refresh token, used to mint a new access token when it expires.
    /// Providers may omit it on refresh, so it is retained across refreshes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Unix seconds at which the access token expires, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

impl OAuthTokens {
    /// Whether the access token is expired (or within `skew_secs` of expiry).
    /// Tokens with no known expiry are treated as still valid.
    #[must_use]
    pub fn is_expired(&self, now_unix: i64, skew_secs: i64) -> bool {
        self.expires_at
            .is_some_and(|exp| now_unix + skew_secs >= exp)
    }
}

/// The outcome of starting an authorization: the URL to send the user to, and
/// the CSRF state and PKCE verifier to carry until the callback.
#[derive(Debug)]
pub struct Authorization {
    /// The provider authorize URL to redirect the user to.
    pub authorize_url: String,
    /// Opaque CSRF token; the callback must present the same value.
    pub csrf_state: String,
    /// PKCE verifier; the callback must present it to exchange the code.
    pub pkce_verifier: String,
}

/// A configured `oauth2` client for the authorization-code grant, with the auth,
/// token, and redirect endpoints all set.
type ConfiguredClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// Builds the `oauth2` client for a provider and app.
fn build_client(provider: &OAuthProvider, app: &OAuthApp) -> Result<ConfiguredClient, Error> {
    let auth_url = AuthUrl::new(provider.auth_url.clone())
        .map_err(|err| Error::connection("invalid OAuth auth URL").with_source(err))?;
    let token_url = TokenUrl::new(provider.token_url.clone())
        .map_err(|err| Error::connection("invalid OAuth token URL").with_source(err))?;
    let redirect_uri = RedirectUrl::new(app.redirect_uri.clone())
        .map_err(|err| Error::connection("invalid OAuth redirect URI").with_source(err))?;

    Ok(BasicClient::new(ClientId::new(app.client_id.clone()))
        .set_client_secret(ClientSecret::new(app.client_secret.clone()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_uri))
}

/// Starts the authorization-code flow: returns the URL to send the user to plus
/// the CSRF state and PKCE verifier the callback must present.
pub fn begin_authorization(
    provider: &OAuthProvider,
    app: &OAuthApp,
) -> Result<Authorization, Error> {
    let client = build_client(provider, app)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();

    let mut request = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(challenge);
    for scope in &provider.scopes {
        request = request.add_scope(Scope::new(scope.clone()));
    }
    for (key, value) in &provider.extra_authorize_params {
        request = request.add_extra_param(key.clone(), value.clone());
    }
    let (authorize_url, csrf) = request.url();

    Ok(Authorization {
        authorize_url: authorize_url.to_string(),
        csrf_state: csrf.secret().clone(),
        pkce_verifier: verifier.secret().clone(),
    })
}

/// Exchanges an authorization `code` (with its PKCE `verifier`) for tokens.
pub async fn exchange_code(
    provider: &OAuthProvider,
    app: &OAuthApp,
    http: &reqwest::Client,
    code: String,
    pkce_verifier: String,
) -> Result<OAuthTokens, Error> {
    let client = build_client(provider, app)?;
    let response = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(&reqwest_client(http.clone()))
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Unauthenticated, "OAuth code exchange failed").with_source(err)
        })?;

    Ok(tokens_from_response(&response, None))
}

/// Refreshes the access token using a stored refresh token.
pub async fn refresh_tokens(
    provider: &OAuthProvider,
    app: &OAuthApp,
    http: &reqwest::Client,
    refresh_token: &str,
) -> Result<OAuthTokens, Error> {
    let client = build_client(provider, app)?;
    let response = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_owned()))
        .request_async(&reqwest_client(http.clone()))
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Unauthenticated, "OAuth token refresh failed").with_source(err)
        })?;

    // A refresh response often omits the refresh token; keep the existing one.
    Ok(tokens_from_response(
        &response,
        Some(refresh_token.to_owned()),
    ))
}

/// Converts an `oauth2` token response into the persisted [`OAuthTokens`],
/// falling back to `existing_refresh` when the response omits a refresh token.
fn tokens_from_response(
    response: &oauth2::basic::BasicTokenResponse,
    existing_refresh: Option<String>,
) -> OAuthTokens {
    let expires_at = response.expires_in().map(|d| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now + d.as_secs() as i64
    });
    OAuthTokens {
        access_token: response.access_token().secret().clone(),
        refresh_token: response
            .refresh_token()
            .map(|t| t.secret().clone())
            .or(existing_refresh),
        expires_at,
    }
}
