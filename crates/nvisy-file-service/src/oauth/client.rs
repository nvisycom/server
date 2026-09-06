//! The [`OAuthClient`]: the authorization-code flow bound to a provider and app.

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

use super::http_client::OAuthHttpClient;
use super::types::{Authorization, OAuthApp, OAuthProvider, OAuthTokens};
use crate::error::{Error, ErrorKind, Result};

/// A configured `oauth2` client for the authorization-code grant, with the auth,
/// token, and redirect endpoints all set.
type ConfiguredClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// An OAuth2 client bound to a provider, an app, and an HTTP client.
///
/// Groups the parameters every flow step shares, so building the authorize URL,
/// exchanging a code, and refreshing a token are methods rather than functions
/// each threading provider/app/http.
#[derive(Clone)]
pub struct OAuthClient {
    provider: OAuthProvider,
    app: OAuthApp,
    http: reqwest::Client,
}

impl OAuthClient {
    /// Binds a provider and app to an HTTP client.
    #[must_use]
    pub fn new(provider: OAuthProvider, app: OAuthApp, http: reqwest::Client) -> Self {
        Self {
            provider,
            app,
            http,
        }
    }

    /// Builds the underlying `oauth2` client with the endpoints set.
    fn build(&self) -> Result<ConfiguredClient> {
        let auth_url = AuthUrl::new(self.provider.auth_url.clone())
            .map_err(|err| Error::connection("invalid OAuth auth URL").with_source(err))?;
        let token_url = TokenUrl::new(self.provider.token_url.clone())
            .map_err(|err| Error::connection("invalid OAuth token URL").with_source(err))?;
        let redirect_uri = RedirectUrl::new(self.app.redirect_uri.clone())
            .map_err(|err| Error::connection("invalid OAuth redirect URI").with_source(err))?;

        Ok(BasicClient::new(ClientId::new(self.app.client_id.clone()))
            .set_client_secret(ClientSecret::new(self.app.client_secret.clone()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_uri))
    }

    /// Starts the authorization-code flow: returns the URL to send the user to
    /// plus the CSRF state and PKCE verifier the callback must present.
    pub fn begin_authorization(&self) -> Result<Authorization> {
        let client = self.build()?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();

        let mut request = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(challenge);
        for scope in &self.provider.scopes {
            request = request.add_scope(Scope::new(scope.clone()));
        }
        for (key, value) in &self.provider.extra_authorize_params {
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
    pub async fn exchange_code(&self, code: String, pkce_verifier: String) -> Result<OAuthTokens> {
        let response = self
            .build()?
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(&OAuthHttpClient::new(self.http.clone()))
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Unauthenticated, "OAuth code exchange failed")
                    .with_source(err)
            })?;

        Ok(tokens_from_response(&response, None))
    }

    /// Refreshes the access token using a stored refresh token.
    pub async fn refresh_tokens(&self, refresh_token: &str) -> Result<OAuthTokens> {
        let response = self
            .build()?
            .exchange_refresh_token(&RefreshToken::new(refresh_token.to_owned()))
            .request_async(&OAuthHttpClient::new(self.http.clone()))
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Unauthenticated, "OAuth token refresh failed")
                    .with_source(err)
            })?;

        // A refresh response often omits the refresh token; keep the existing one.
        Ok(tokens_from_response(
            &response,
            Some(refresh_token.to_owned()),
        ))
    }
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
