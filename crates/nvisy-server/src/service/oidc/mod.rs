//! OIDC sign-in service.
//!
//! Runs the OpenID Connect authorization-code flow for the deployment's
//! configured providers (Google, Microsoft). A sign-in has two steps, matching
//! the two handler endpoints:
//!
//! - [`begin`](OidcService::begin) discovers the provider, builds the authorize
//!   URL, and returns it alongside the CSRF state, PKCE verifier, and nonce the
//!   caller must stash server-side until the callback.
//! - [`complete`](OidcService::complete) exchanges the code and verifies the
//!   returned ID token (signature, `aud`, `iss`, `exp`, and `nonce`), yielding
//!   the provider's stable subject and asserted email.
//!
//! Provider metadata (and its JWKS) is discovered per sign-in rather than cached:
//! a sign-in is infrequent relative to the discovery cost, and re-discovering
//! avoids serving a stale signing-key set after a provider rotates keys.

use std::sync::Arc;
use std::time::Duration;

use nvisy_postgres::types::IdentityProvider;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenResponse, reqwest,
};

mod config;
mod error;

pub use config::{OidcConfig, ResolvedOidcProvider};
pub use error::OidcError;

/// A configured OIDC provider ready to run sign-ins.
///
/// Holds the resolved credentials and issuer; the per-request `openidconnect`
/// client is built after discovery inside [`begin`](OidcService::begin) and
/// [`complete`](OidcService::complete).
#[derive(Debug, Clone)]
struct OidcProvider {
    provider: IdentityProvider,
    client_id: ClientId,
    client_secret: ClientSecret,
    redirect_uri: RedirectUrl,
    issuer: IssuerUrl,
}

impl OidcProvider {
    /// Validates a resolved provider's URLs up front, so a misconfigured issuer
    /// or redirect fails at startup rather than on the first sign-in.
    fn from_resolved(resolved: ResolvedOidcProvider) -> Result<Self, OidcError> {
        Ok(Self {
            provider: resolved.provider,
            client_id: ClientId::new(resolved.client_id),
            client_secret: ClientSecret::new(resolved.client_secret),
            redirect_uri: RedirectUrl::new(resolved.redirect_uri)
                .map_err(|e| OidcError::config("invalid redirect URI", e))?,
            issuer: IssuerUrl::new(resolved.issuer)
                .map_err(|e| OidcError::config("invalid issuer URL", e))?,
        })
    }
}

/// The authorization-request secrets a sign-in must carry from `begin` to
/// `complete`. Stashed server-side (never round-tripped through the browser, or
/// PKCE and the nonce would be defeated).
#[derive(Debug)]
pub struct OidcAuthorization {
    /// The provider authorize URL to send the user to.
    pub authorize_url: String,
    /// The opaque CSRF state token the provider echoes back; also the stash key.
    pub csrf_state: String,
    /// The PKCE verifier to present when exchanging the code.
    pub pkce_verifier: String,
    /// The nonce bound into the ID token, verified on the way back.
    pub nonce: String,
}

/// The verified result of a completed OIDC sign-in.
pub struct OidcIdentity {
    /// The provider's stable subject (`sub`) claim — the identity's durable key.
    /// Each provider is pinned to a single issuer, so the subject is unique within
    /// the provider.
    pub subject: String,
    /// The email the provider asserted, if any (and if the scope was granted).
    pub email: Option<String>,
    /// Whether the provider asserts the email is verified (`email_verified`
    /// claim). Only a verified email is a safe basis for linking to an existing
    /// account — an unverified one could be an address the user does not control.
    pub email_verified: bool,
}

/// A discovered `openidconnect` client, with its endpoints resolved from the
/// provider metadata and the redirect URI set. The generic parameters record
/// which endpoints are present; auth + token are guaranteed by construction,
/// the rest are "maybe set" per discovery.
type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// How long to wait to establish a TCP connection to a provider endpoint.
const OIDC_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Overall deadline for a single provider HTTP call (discovery, token exchange).
const OIDC_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Runs OIDC sign-ins for the deployment's configured providers.
///
/// Cheaply cloneable: it holds a shared `reqwest` client and the (small) set of
/// configured providers.
#[derive(Clone)]
pub struct OidcService {
    http: reqwest::Client,
    providers: Arc<Vec<OidcProvider>>,
    /// Frontend origins (`scheme://host[:port]`) the sign-in callback may redirect
    /// to. A caller-supplied redirect must match one of these, or it is refused —
    /// so the flow's minted session token cannot be sent to an attacker host.
    allowed_redirect_origins: Arc<Vec<String>>,
    /// Custom URL schemes (e.g. `nvisy`) the callback may deep-link to for native
    /// app (desktop) auth. A redirect whose scheme matches one of these is a
    /// desktop flow (token in the deep-link), distinct from a web origin (cookie).
    allowed_redirect_schemes: Arc<Vec<String>>,
}

/// What kind of allowed redirect target a `redirectUri` is, deciding how the
/// sign-in callback delivers its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectKind {
    /// An allow-listed web origin: the callback sets a session cookie (web flow).
    WebOrigin,
    /// An allow-listed custom scheme: the callback deep-links an API token to the
    /// native app (desktop flow).
    DesktopScheme,
}

impl OidcService {
    /// Builds the service from the deployment's OIDC configuration.
    ///
    /// Only fully-configured providers are enabled; the rest are simply absent.
    /// The `openidconnect` guidance requires the HTTP client to disable redirect
    /// following (an SSRF hardening: a token/userinfo endpoint must not be
    /// followed to an attacker-chosen location).
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built or a configured
    /// provider has an invalid issuer or redirect URI.
    pub fn from_config(config: &OidcConfig) -> Result<Self, OidcError> {
        // Bound the outbound calls to the provider (discovery, token exchange):
        // an unresponsive or slow provider must not pin a request handler open
        // indefinitely.
        let http = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(OIDC_HTTP_CONNECT_TIMEOUT)
            .timeout(OIDC_HTTP_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| OidcError::config("failed to build OIDC HTTP client", e))?;

        let providers = config
            .resolved_providers()
            .into_iter()
            .map(OidcProvider::from_resolved)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            http,
            providers: Arc::new(providers),
            allowed_redirect_origins: Arc::new(config.allowed_redirect_origins()),
            allowed_redirect_schemes: Arc::new(config.desktop_allowed_redirect_schemes()),
        })
    }

    /// Classifies a caller-supplied `redirect_uri` as a permitted redirect target,
    /// or `None` if it is not allow-listed.
    ///
    /// A web target's origin (`scheme://host[:port]`) must exactly match a
    /// configured allowed origin ([`WebOrigin`](RedirectKind::WebOrigin)); a
    /// desktop target's custom scheme must match a configured desktop scheme
    /// ([`DesktopScheme`](RedirectKind::DesktopScheme)). Anything else — an
    /// unparseable URL, an un-allow-listed origin, an unknown scheme — is refused,
    /// because the callback carries the minted token and must never deliver it to
    /// a target the deployment did not sanction.
    #[must_use]
    pub fn classify_redirect(&self, redirect_uri: &str) -> Option<RedirectKind> {
        let url = url::Url::parse(redirect_uri).ok()?;

        // A tuple (http/https) origin is a web target; match it against the origin
        // allow-list. `Url::origin()` yields an opaque origin for a custom scheme,
        // which is never a web match.
        let origin = url.origin();
        if origin.is_tuple() {
            let ascii_origin = origin.ascii_serialization().to_ascii_lowercase();
            return self
                .allowed_redirect_origins
                .contains(&ascii_origin)
                .then_some(RedirectKind::WebOrigin);
        }

        // Otherwise it may be a desktop deep-link: its scheme must be allow-listed.
        let scheme = url.scheme().to_ascii_lowercase();
        self.allowed_redirect_schemes
            .contains(&scheme)
            .then_some(RedirectKind::DesktopScheme)
    }

    /// Whether `redirect_uri` is a permitted post-sign-in redirect target (web or
    /// desktop). Used to gate a flow at start; the callback re-classifies to
    /// decide how to deliver the result.
    #[must_use]
    pub fn is_redirect_allowed(&self, redirect_uri: &str) -> bool {
        self.classify_redirect(redirect_uri).is_some()
    }

    /// Looks up a configured provider by its identity kind.
    fn provider(&self, provider: IdentityProvider) -> Result<&OidcProvider, OidcError> {
        self.providers
            .iter()
            .find(|p| p.provider == provider)
            .ok_or(OidcError::ProviderNotConfigured(provider))
    }

    /// Discovers the provider metadata and builds a client for it, with the
    /// redirect URI set.
    async fn discover(&self, provider: &OidcProvider) -> Result<DiscoveredClient, OidcError> {
        let metadata = CoreProviderMetadata::discover_async(provider.issuer.clone(), &self.http)
            .await
            .map_err(|e| OidcError::discovery(provider.provider, e))?;

        let client = CoreClient::from_provider_metadata(
            metadata,
            provider.client_id.clone(),
            Some(provider.client_secret.clone()),
        )
        .set_redirect_uri(provider.redirect_uri.clone());

        Ok(client)
    }

    /// Begins a sign-in: builds the authorize URL and the secrets to stash.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not configured or discovery fails.
    pub async fn begin(&self, provider: IdentityProvider) -> Result<OidcAuthorization, OidcError> {
        let provider = self.provider(provider)?;
        let client = self.discover(provider).await?;

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (authorize_url, csrf_state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("openid".to_owned()))
            .add_scope(Scope::new("email".to_owned()))
            .add_scope(Scope::new("profile".to_owned()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        Ok(OidcAuthorization {
            authorize_url: authorize_url.to_string(),
            csrf_state: csrf_state.secret().to_owned(),
            pkce_verifier: pkce_verifier.into_secret(),
            nonce: nonce.secret().to_owned(),
        })
    }

    /// Completes a sign-in: exchanges the code and verifies the ID token against
    /// the stashed PKCE verifier and nonce, returning the provider's subject and
    /// asserted email.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not configured, the code exchange
    /// fails, no ID token is returned, or ID-token verification fails (bad
    /// signature, `aud`, `iss`, `exp`, or `nonce`).
    pub async fn complete(
        &self,
        provider: IdentityProvider,
        code: String,
        pkce_verifier: String,
        nonce: String,
    ) -> Result<OidcIdentity, OidcError> {
        let provider_config = self.provider(provider)?;
        let client = self.discover(provider_config).await?;

        let token_response = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|e| OidcError::exchange(provider, e.to_string()))?
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(&self.http)
            .await
            .map_err(|e| OidcError::exchange(provider, e.to_string()))?;

        let id_token = token_response
            .id_token()
            .ok_or(OidcError::MissingIdToken(provider))?;

        let nonce = Nonce::new(nonce);
        let claims = id_token
            .claims(&client.id_token_verifier(), &nonce)
            .map_err(|e| OidcError::verification(provider, e.to_string()))?;

        let email = claims.email().map(|email| email.as_str().to_owned());

        Ok(OidcIdentity {
            subject: claims.subject().as_str().to_owned(),
            email,
            // Absent claim is treated as unverified: never assume a verification
            // the provider did not assert.
            email_verified: claims.email_verified().unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service_with(origins: &[&str], schemes: &[&str]) -> OidcService {
        OidcService {
            http: reqwest::Client::new(),
            providers: Arc::new(Vec::new()),
            allowed_redirect_origins: Arc::new(origins.iter().map(|o| o.to_string()).collect()),
            allowed_redirect_schemes: Arc::new(schemes.iter().map(|s| s.to_string()).collect()),
        }
    }

    fn service_with_origins(origins: &[&str]) -> OidcService {
        service_with(origins, &[])
    }

    #[test]
    fn redirect_allowed_only_for_matching_origin() {
        let oidc = service_with_origins(&["https://app.example.com"]);

        // Same origin, any path/query is allowed.
        assert!(oidc.is_redirect_allowed("https://app.example.com/signed-in?x=1"));
        // A different host, scheme, or port is a different origin.
        assert!(!oidc.is_redirect_allowed("https://evil.example.com/"));
        assert!(!oidc.is_redirect_allowed("http://app.example.com/"));
        assert!(!oidc.is_redirect_allowed("https://app.example.com:8443/"));
        // A subdomain is not the configured origin.
        assert!(!oidc.is_redirect_allowed("https://x.app.example.com/"));
    }

    #[test]
    fn redirect_rejected_when_no_origins_configured() {
        let oidc = service_with_origins(&[]);
        assert!(!oidc.is_redirect_allowed("https://app.example.com/"));
    }

    #[test]
    fn redirect_rejects_unparseable_and_opaque_origins() {
        let oidc = service_with_origins(&["https://app.example.com"]);
        assert!(!oidc.is_redirect_allowed("not a url"));
        assert!(!oidc.is_redirect_allowed("data:text/html,evil"));
        // A relative path has no origin to match.
        assert!(!oidc.is_redirect_allowed("/signed-in"));
    }

    #[test]
    fn classify_distinguishes_web_origins_and_desktop_schemes() {
        let oidc = service_with(&["https://app.example.com"], &["nvisy"]);

        // A web origin classifies as a web target (cookie flow).
        assert_eq!(
            oidc.classify_redirect("https://app.example.com/done"),
            Some(RedirectKind::WebOrigin)
        );
        // The allow-listed custom scheme classifies as a desktop target.
        assert_eq!(
            oidc.classify_redirect("nvisy://auth/callback"),
            Some(RedirectKind::DesktopScheme)
        );
        // An un-allow-listed scheme or origin is refused.
        assert_eq!(oidc.classify_redirect("other://auth/callback"), None);
        assert_eq!(oidc.classify_redirect("https://evil.example.com/"), None);
    }

    #[test]
    fn desktop_scheme_is_not_matched_when_none_configured() {
        // With no desktop schemes, a custom-scheme redirect is refused even though
        // web origins are allowed.
        let oidc = service_with_origins(&["https://app.example.com"]);
        assert_eq!(oidc.classify_redirect("nvisy://auth/callback"), None);
    }
}
