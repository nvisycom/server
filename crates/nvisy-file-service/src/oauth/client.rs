//! The [`OAuthClient`]: the authorization-code flow bound to a provider and app.

use jiff::Timestamp;
use nvisy_core::http::OAuthHttpClient;
use nvisy_core::{Error, ErrorKind, Result, Secret};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

use super::types::{Authorization, OAuthApp, OAuthProvider, OAuthTokens};

/// A configured `oauth2` client for the authorization-code grant, with the auth,
/// token, and redirect endpoints all set.
type ConfiguredClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// An `OAuth2` client bound to a provider, an app, and the shared OAuth HTTP
/// transport, built once and reused for every flow step.
///
/// Building validates the provider's endpoints and the app's redirect URI, so a
/// misconfigured app fails when the service is built rather than mid-flow.
#[derive(Clone)]
pub struct OAuthClient {
    client: ConfiguredClient,
    provider: OAuthProvider,
    http: OAuthHttpClient,
}

impl OAuthClient {
    /// Builds the client for `provider` and `app`, sending token requests through
    /// `http`.
    ///
    /// # Errors
    ///
    /// Returns an `Invalid` error if the auth URL, token URL, or redirect URI is
    /// not a valid URL.
    pub fn new(provider: OAuthProvider, app: &OAuthApp, http: OAuthHttpClient) -> Result<Self> {
        let auth_url = AuthUrl::new(provider.auth_url.clone())
            .map_err(|err| Error::invalid("invalid OAuth auth URL").with_source(err))?;
        let token_url = TokenUrl::new(provider.token_url.clone())
            .map_err(|err| Error::invalid("invalid OAuth token URL").with_source(err))?;
        let redirect_uri = RedirectUrl::new(app.redirect_uri.clone())
            .map_err(|err| Error::invalid("invalid OAuth redirect URI").with_source(err))?;

        let client = BasicClient::new(ClientId::new(app.client_id.clone()))
            .set_client_secret(ClientSecret::new(app.client_secret.expose().clone()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_uri);
        Ok(Self {
            client,
            provider,
            http,
        })
    }

    /// Starts the authorization-code flow: returns the URL to send the user to
    /// plus the CSRF state and PKCE verifier the callback must present.
    ///
    #[must_use]
    pub fn begin_authorization(&self) -> Authorization {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();

        let mut request = self
            .client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(challenge);
        for scope in &self.provider.scopes {
            request = request.add_scope(Scope::new(scope.clone()));
        }
        for (key, value) in &self.provider.extra_authorize_params {
            request = request.add_extra_param(key.clone(), value.clone());
        }
        let (authorize_url, csrf) = request.url();

        Authorization {
            authorize_url: authorize_url.to_string(),
            csrf_state: csrf.secret().clone(),
            pkce_verifier: verifier.secret().clone(),
        }
    }

    /// Exchanges an authorization `code` (with its PKCE `verifier`) for tokens.
    ///
    /// # Errors
    ///
    /// Returns an `Unauthenticated` error if the code exchange request fails or
    /// is rejected by the provider.
    pub async fn exchange_code(&self, code: String, pkce_verifier: String) -> Result<OAuthTokens> {
        let response = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(&self.http)
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Unauthenticated, "OAuth code exchange failed")
                    .with_source(err)
            })?;

        Ok(tokens_from_response(&response, None))
    }

    /// Refreshes the access token using a stored refresh token.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`refresh_with_scopes`](Self::refresh_with_scopes).
    pub async fn refresh_tokens(&self, refresh_token: &str) -> Result<OAuthTokens> {
        self.refresh_with_scopes(refresh_token, &[]).await
    }

    /// Refreshes the access token for a specific set of `scopes` (a subset of the
    /// original grant), yielding a token whose audience matches those scopes.
    ///
    /// Used to mint a resource-specific token from the same refresh token without
    /// re-consent — e.g. a SharePoint-audience token for the `OneDrive` file picker,
    /// distinct from the Graph token the connector uses. An empty `scopes` refreshes
    /// with the originally granted scopes (see [`refresh_tokens`](Self::refresh_tokens)).
    ///
    /// # Errors
    ///
    /// Returns an `Unauthenticated` error if the refresh request fails or is
    /// rejected by the provider.
    pub async fn refresh_with_scopes(
        &self,
        refresh_token: &str,
        scopes: &[String],
    ) -> Result<OAuthTokens> {
        let refresh = RefreshToken::new(refresh_token.to_owned());
        let mut request = self.client.exchange_refresh_token(&refresh);
        for scope in scopes {
            request = request.add_scope(Scope::new(scope.clone()));
        }
        let response = request.request_async(&self.http).await.map_err(|err| {
            Error::new(ErrorKind::Unauthenticated, "OAuth token refresh failed").with_source(err)
        })?;

        // A refresh response often omits the refresh token; keep the existing one.
        Ok(tokens_from_response(
            &response,
            Some(Secret::new(refresh_token.to_owned())),
        ))
    }
}

/// Converts an `oauth2` token response into the persisted [`OAuthTokens`],
/// falling back to `existing_refresh` when the response omits a refresh token.
fn tokens_from_response(
    response: &oauth2::basic::BasicTokenResponse,
    existing_refresh: Option<Secret<String>>,
) -> OAuthTokens {
    let now = Timestamp::now().as_second();
    let expires_at = response
        .expires_in()
        .map(|lifetime| now.saturating_add(i64::try_from(lifetime.as_secs()).unwrap_or(i64::MAX)));
    OAuthTokens {
        access_token: Secret::new(response.access_token().secret().clone()),
        refresh_token: response
            .refresh_token()
            .map(|t| Secret::new(t.secret().clone()))
            .or(existing_refresh),
        expires_at,
    }
}
