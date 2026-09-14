//! OIDC sign-in service.
//!
//! Runs the `OpenID` Connect authorization-code flows (sign-in, account link, and
//! step-up re-authentication) for the deployment's configured providers (Google,
//! Microsoft), and owns the browser-driven flow orchestration: stashing and
//! consuming the single-use flow state, resolving the identity to an account, and
//! minting (or re-authenticating) a session.
//!
//! The protocol primitives underneath are:
//!
//! - [`begin`](OidcService::begin) discovers the provider, builds the authorize
//!   URL, and returns it alongside the CSRF state, PKCE verifier, and nonce the
//!   flow must stash server-side until the callback.
//! - [`complete`](OidcService::complete) exchanges the code and verifies the
//!   returned ID token (signature, `aud`, `iss`, `exp`, and `nonce`), yielding
//!   the provider's stable subject and asserted email.
//!
//! [`begin_flow`](OidcService::begin_flow), [`consume_flow`](OidcService::consume_flow),
//! and [`run_flow`](OidcService::run_flow) wrap those primitives with the flow
//! state and the callback action; the handler stays thin transport (cookies,
//! redirects). The expensive, immutable configuration (the `reqwest` client and
//! parsed providers) is loaded once into [`OidcConfigured`] and stored on the
//! application state; the per-request [`OidcService`] composes it with the ambient
//! collaborators it drives.
//!
//! Provider metadata (and its JWKS) is discovered per sign-in rather than cached:
//! a sign-in is infrequent relative to the discovery cost, and re-discovering
//! avoids serving a stale signing-key set after a provider rotates keys.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use nvisy_nats::NatsClient;
use nvisy_nats::kv::{
    OidcStateBucket as OidcStateKvBucket, OidcStateKey, ReauthProofBucket as ReauthProofKvBucket,
    ReauthProofKey,
};
use nvisy_postgres::model::Account;
use nvisy_postgres::query::{AccountIdentityRepository, AccountRepository};
use nvisy_postgres::types::IdentityProvider;
use nvisy_postgres::{PgClient, PgConn};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
    TokenResponse, reqwest,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod config;
mod error;

pub use config::{OidcConfig, ResolvedOidcProvider};
pub use error::OidcError;

use crate::extract::SecurityContext;
use crate::handler::request::OidcCallbackQuery;
use crate::response::{ErrorKind, Result};
use crate::service::{AccountProvisioner, AuthIssuer};

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

/// The startup-loaded, immutable OIDC configuration: the shared `reqwest` client,
/// the configured providers, and the redirect allow-lists.
///
/// Built once by [`OidcConfigured::from_config`] and stored behind an `Arc` on the
/// application state, so the per-request [`OidcService`] is assembled by cloning
/// this handle and composing the (also `Arc`-backed) collaborators around it.
struct OidcCore {
    http: reqwest::Client,
    providers: Vec<OidcProvider>,
    /// Frontend origins (`scheme://host[:port]`) the sign-in callback may redirect
    /// to. A caller-supplied redirect must match one of these, or it is refused —
    /// so the flow's minted session token cannot be sent to an attacker host.
    allowed_redirect_origins: Vec<String>,
    /// Custom URL schemes (e.g. `nvisy`) the callback may deep-link to for native
    /// app (desktop) auth. A redirect whose scheme matches one of these is a
    /// desktop flow (token in the deep-link), distinct from a web origin (cookie).
    allowed_redirect_schemes: Vec<String>,
}

/// Runs OIDC sign-ins, links, and step-up re-authentications for the deployment's
/// configured providers, and owns the browser-driven flow orchestration.
///
/// Assembled per request from the shared startup config (the expensive, immutable
/// `reqwest` client and parsed providers, built once and held behind an `Arc` in
/// [`OidcConfigured`]) plus the `Arc`-backed collaborators the flow drives:
/// [`NatsClient`] (flow-state and reauth-proof KV), [`AuthIssuer`] (session mint),
/// [`AccountProvisioner`] (account resolution/linking), and the [`PgClient`].
/// Cloning is cheap — every field is `Arc`-backed.
#[derive(Clone)]
pub struct OidcService {
    core: Arc<OidcCore>,
    postgres: PgClient,
    nats: NatsClient,
    issuer: AuthIssuer,
    provisioner: AccountProvisioner,
}

/// The startup-built OIDC configuration handle stored on the application state.
///
/// Held behind an `Arc` so the per-request [`OidcService`] is composed by cloning
/// it and adding the ambient collaborators, rather than rebuilding the `reqwest`
/// client and re-parsing providers on every request.
#[derive(Clone)]
pub struct OidcConfigured {
    core: Arc<OidcCore>,
}

/// What an in-flight OIDC flow is for. All three share the same authorize +
/// callback machinery; only the callback's action differs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OidcPurpose {
    /// Ordinary sign-in: resolve the identity to an account (provisioning or
    /// auto-linking as needed) and mint a session.
    SignIn,
    /// Link the provider to an already-authenticated account. Requires a fresh
    /// step-up proof, so the flow carries the account it links to.
    Link { account_id: Uuid },
    /// Step-up re-authentication for `account_id`: prove current control of a
    /// linked provider identity and mint a single-use proof (no session, no
    /// linking). Gates credential-adding actions against a merely-stolen session.
    Reauth { account_id: Uuid },
}

/// The stashed state of an in-flight OIDC flow, held between `begin_flow` and the
/// callback in the [`OidcStateBucket`]. Keyed by the CSRF state token.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OidcFlowState {
    /// The provider the flow is against.
    provider: IdentityProvider,
    /// The PKCE verifier to present when exchanging the code.
    pkce_verifier: String,
    /// The nonce bound into the ID token, verified on the way back.
    nonce: String,
    /// Frontend URL to send the user back to once done, if provided.
    redirect_uri: Option<String>,
    /// What the flow is for, and any account it is bound to.
    purpose: OidcPurpose,
}

/// The OIDC-state bucket pinned to this server's flow-state value.
type OidcStateBucket = OidcStateKvBucket<OidcFlowState>;

/// What a step-up re-authentication proof attests: the account it authorizes a
/// credential-adding action for.
///
/// A proof is single-use and short-lived (the bucket TTL), and authorizes exactly
/// one credential-add for its account — whichever add (set a first password, or
/// link a provider) presents it first. It is deliberately *not* scoped to a
/// specific action: the guarantee it carries is "the holder just proved current
/// control of a provider identity on this account", which is what every
/// credential-add needs. Single-use + TTL + account-binding keep that from being
/// a standing capability; consuming it atomically keeps it from being used twice.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReauthProof {
    account_id: Uuid,
}

/// The reauth-proof bucket pinned to this server's proof value.
type ReauthProofBucket = ReauthProofKvBucket<ReauthProof>;

/// The result of a completed callback flow, by purpose. The handler turns this
/// into the browser response (a cookie, a deep-link, or a fragment redirect).
pub enum CallbackOutcome {
    /// A web sign-in: the minted session JWT, delivered to the browser as an
    /// `HttpOnly` session cookie set on the callback redirect.
    SignedIn { jwt: String },
    /// A native-app (desktop) sign-in: the minted `app` token, delivered to the
    /// app in the callback's deep-link query (`?token=…`), never a cookie.
    DesktopSignedIn { jwt: String },
    /// A provider was linked to the account. No token; just an outcome.
    Linked,
    /// A step-up re-authentication: the single-use proof is handed back for the
    /// frontend to present to the credential-adding action.
    Reauthed { proof: String },
}

impl CallbackOutcome {
    /// A short label for logging.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SignedIn { .. } => "sign_in",
            Self::DesktopSignedIn { .. } => "sign_in_desktop",
            Self::Linked => "link",
            Self::Reauthed { .. } => "reauth",
        }
    }
}

/// Generates an opaque, unguessable proof/token value (URL-safe base64 of 32
/// random bytes), suitable for a NATS KV key.
fn generate_proof_token() -> String {
    use base64::Engine;
    use rand::Rng;

    // Security-critical: the proof value is a bearer capability, so its bytes must
    // come from a cryptographically secure RNG. `rand::rng()` returns the
    // thread-local CSPRNG (`ThreadRng`); do not swap it for a non-cryptographic
    // generator.
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
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

impl OidcConfigured {
    /// Builds the startup OIDC configuration from the deployment's config.
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
            core: Arc::new(OidcCore {
                http,
                providers,
                allowed_redirect_origins: config.allowed_redirect_origins(),
                allowed_redirect_schemes: config.desktop_allowed_redirect_schemes(),
            }),
        })
    }

    /// Assembles a per-request [`OidcService`] from this configuration and the
    /// ambient collaborators the flow drives.
    #[must_use]
    pub fn service(
        &self,
        postgres: PgClient,
        nats: NatsClient,
        issuer: AuthIssuer,
        provisioner: AccountProvisioner,
    ) -> OidcService {
        OidcService {
            core: self.core.clone(),
            postgres,
            nats,
            issuer,
            provisioner,
        }
    }
}

impl OidcCore {
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
    fn classify_redirect(&self, redirect_uri: &str) -> Option<RedirectKind> {
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

    /// Whether `redirect_uri` is a permitted redirect target (web or desktop).
    #[cfg(test)]
    fn is_redirect_allowed(&self, redirect_uri: &str) -> bool {
        self.classify_redirect(redirect_uri).is_some()
    }
}

impl OidcService {
    /// The identity providers this deployment has configured for OIDC sign-in, in
    /// configuration order. Only fully-configured providers are present, so a
    /// client can offer exactly the sign-in buttons that will work.
    #[must_use]
    pub fn configured_providers(&self) -> Vec<IdentityProvider> {
        self.core.providers.iter().map(|p| p.provider).collect()
    }

    /// Classifies a caller-supplied `redirect_uri` as a permitted redirect target,
    /// or `None` if it is not allow-listed.
    ///
    /// A web target's origin (`scheme://host[:port]`) must exactly match a
    /// configured allowed origin ([`WebOrigin`](RedirectKind::WebOrigin)); a
    /// desktop target's custom scheme must match a configured desktop scheme
    /// ([`DesktopScheme`](RedirectKind::DesktopScheme)). Anything else is refused,
    /// because the callback carries the minted token and must never deliver it to a
    /// target the deployment did not sanction.
    #[must_use]
    pub fn classify_redirect(&self, redirect_uri: &str) -> Option<RedirectKind> {
        self.core.classify_redirect(redirect_uri)
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
        self.core
            .providers
            .iter()
            .find(|p| p.provider == provider)
            .ok_or(OidcError::ProviderNotConfigured(provider))
    }

    /// Discovers the provider metadata and builds a client for it, with the
    /// redirect URI set.
    async fn discover(&self, provider: &OidcProvider) -> Result<DiscoveredClient, OidcError> {
        let metadata =
            CoreProviderMetadata::discover_async(provider.issuer.clone(), &self.core.http)
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
            .request_async(&self.core.http)
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

    /// Begins an authorization (sign-in, link, or reauth) and stashes the flow
    /// state, returning the provider authorize URL.
    ///
    /// Rejects a redirect target that is not an allow-listed frontend origin
    /// *before* starting the flow: the callback carries the minted session token
    /// (or a reauth proof), so it must never be sent to a caller-chosen host.
    pub async fn begin_flow(
        &self,
        provider: IdentityProvider,
        redirect_uri: Option<String>,
        purpose: OidcPurpose,
    ) -> Result<String> {
        if let Some(redirect_uri) = &redirect_uri
            && !self.is_redirect_allowed(redirect_uri)
        {
            return Err(ErrorKind::BadRequest.with_message("redirectUri is not an allowed origin"));
        }

        let OidcAuthorization {
            authorize_url,
            csrf_state,
            pkce_verifier,
            nonce,
        } = self.begin(provider).await?;

        let flow = OidcFlowState {
            provider,
            pkce_verifier,
            nonce,
            redirect_uri,
            purpose,
        };
        let store = self.nats.kv_store::<OidcStateBucket>().await?;
        store.put(&OidcStateKey(csrf_state), &flow).await?;

        Ok(authorize_url)
    }

    /// Consumes the pending flow state single-use (atomically) and returns it.
    /// Split from [`run_flow`](Self::run_flow) so the caller recovers the flow's
    /// `redirect_uri` before any fallible step, and can honor it even when that
    /// step fails. Returns the caller's redirect target alongside the state.
    pub async fn consume_flow(&self, query: &OidcCallbackQuery) -> Result<ConsumedFlow> {
        // Validate the state before touching the store so a malformed value maps to
        // a clean BadRequest rather than a KV error.
        let key = OidcStateKey::from_str(&query.state)
            .map_err(|_| ErrorKind::BadRequest.with_message("Invalid authorization state"))?;

        // `take` consumes single-use and atomically, so two concurrent callbacks
        // carrying the same state cannot both proceed. A missing entry means an
        // unknown, expired, or already-consumed state.
        let store = self.nats.kv_store::<OidcStateBucket>().await?;
        let flow = store.take(&key).await?.ok_or_else(|| {
            ErrorKind::BadRequest.with_message("Invalid or expired authorization")
        })?;

        Ok(ConsumedFlow {
            redirect_uri: flow.redirect_uri.clone(),
            flow,
        })
    }

    /// Runs a consumed flow's work: exchange and verify the code, then perform the
    /// flow's purpose (sign in, link, or mint a reauth proof). The flow state has
    /// already been consumed by [`consume_flow`](Self::consume_flow), so its
    /// `redirect_uri` is the caller's and is applied by the callback whether this
    /// succeeds or fails.
    pub async fn run_flow(
        &self,
        security: SecurityContext,
        flow: ConsumedFlow,
        query: OidcCallbackQuery,
    ) -> Result<CallbackOutcome> {
        let flow = flow.flow;

        // A denial (or any provider error) arrives with `error` and no `code`.
        if let Some(error) = query.error {
            return Err(ErrorKind::Unauthorized
                .with_message("Authorization was denied")
                .with_context(error));
        }
        let code = query.code.ok_or_else(|| {
            ErrorKind::BadRequest.with_message("Authorization callback missing code")
        })?;

        // Exchange the code and verify the ID token (signature, aud, iss, exp,
        // nonce).
        let identity = self
            .complete(flow.provider, code, flow.pkce_verifier, flow.nonce)
            .await?;

        let mut conn = self.postgres.get_connection().await?;

        match flow.purpose {
            OidcPurpose::SignIn => {
                let account = self
                    .provisioner
                    .resolve(&mut conn, flow.provider, identity)
                    .await?;
                Self::gate_account_status(&account)?;

                // The redirect target decides how the session is delivered. A
                // desktop deep-link scheme gets a long-lived `app` token in the
                // callback's URL query; a web origin gets an HttpOnly session
                // cookie. The target was already allow-listed at flow start; a
                // `None` here means it is neither kind (which `begin_flow` would
                // have rejected), so default to the web cookie path.
                let kind = flow
                    .redirect_uri
                    .as_deref()
                    .and_then(|uri| self.classify_redirect(uri));

                if kind == Some(RedirectKind::DesktopScheme) {
                    let jwt = self
                        .issuer
                        .issue_app_token(&mut conn, &account, security)
                        .await?;
                    Ok(CallbackOutcome::DesktopSignedIn { jwt })
                } else {
                    // OIDC web sign-in delivers a remembered browser session as an
                    // HttpOnly cookie set on the callback redirect — the token
                    // never appears in the URL.
                    let jwt = self
                        .issuer
                        .issue_web_session(&mut conn, &account, true, security)
                        .await?;
                    Ok(CallbackOutcome::SignedIn { jwt })
                }
            }
            OidcPurpose::Link { account_id } => {
                // A suspended or deleted account must not add a credential, so gate
                // its status before linking — mirroring the sign-in path.
                Self::gate_account_status(&Self::load_account(&mut conn, account_id).await?)?;
                self.provisioner
                    .link(&mut conn, account_id, flow.provider, identity)
                    .await?;
                Ok(CallbackOutcome::Linked)
            }
            OidcPurpose::Reauth { account_id } => {
                // A suspended or deleted account must not mint a step-up proof (it
                // would gate a later credential-adding action anyway), so refuse it
                // up front.
                Self::gate_account_status(&Self::load_account(&mut conn, account_id).await?)?;

                // The verified identity must belong to the account being re-authed:
                // proving control of *some* provider account is not enough, it must
                // be one linked here.
                let matches = conn
                    .find_identity_by_subject(flow.provider, &identity.subject)
                    .await?
                    .is_some_and(|linked| linked.account_id == account_id);
                if !matches {
                    return Err(ErrorKind::Unauthorized
                        .with_message("Re-authentication did not match a linked identity"));
                }
                let proof = self.mint_reauth_proof(account_id).await?;
                Ok(CallbackOutcome::Reauthed { proof })
            }
        }
    }

    /// Loads the account for a `Link`/`Reauth` flow (whose `account_id` comes from
    /// the caller's own session), or a `NotFound` if it is gone.
    async fn load_account(conn: &mut PgConn, account_id: uuid::Uuid) -> Result<Account> {
        conn.find_account_by_id(account_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message("Account not found"))
    }

    /// Refuses a sign-in for a suspended or deleted account, before any session
    /// token is minted — mirroring what password login gates on.
    pub fn gate_account_status(account: &Account) -> Result<()> {
        if account.is_suspended() {
            return Err(ErrorKind::Forbidden.with_message("Account is suspended"));
        }
        if account.is_deleted() {
            return Err(ErrorKind::Forbidden.with_message("Account has been deleted"));
        }
        Ok(())
    }

    /// Mints and stores a single-use step-up proof for `account_id`, returning its
    /// opaque key. The proof is short-lived (the bucket's TTL) and consumed by the
    /// credential-adding action.
    async fn mint_reauth_proof(&self, account_id: Uuid) -> Result<String> {
        let proof = generate_proof_token();
        let store = self.nats.kv_store::<ReauthProofBucket>().await?;
        store
            .put(&ReauthProofKey(proof.clone()), &ReauthProof { account_id })
            .await?;
        Ok(proof)
    }

    /// Consumes a step-up re-authentication proof: verifies it exists and is for
    /// `account_id`, then deletes it (single-use). Returns an error if the proof is
    /// missing, expired, already used, or for a different account.
    ///
    /// Required by every credential-adding action so a merely-stolen session (with
    /// no way to complete a fresh provider re-auth) cannot mint a durable
    /// credential.
    pub async fn consume_reauth_proof(&self, account_id: Uuid, proof: &str) -> Result<()> {
        let key = ReauthProofKey::from_str(proof)
            .map_err(|_| ErrorKind::Unauthorized.with_message("Invalid re-authentication proof"))?;
        let store = self.nats.kv_store::<ReauthProofBucket>().await?;
        // Consume single-use and atomically (`take`), so the same proof cannot be
        // used twice by concurrent requests to add two credentials.
        let stored = store.take(&key).await?.ok_or_else(|| {
            ErrorKind::Unauthorized.with_message("Re-authentication required or expired")
        })?;

        if stored.account_id != account_id {
            return Err(ErrorKind::Unauthorized
                .with_message("Re-authentication proof does not match this account"));
        }
        Ok(())
    }
}

/// A consumed OIDC flow, carrying the caller's redirect target separately so the
/// callback can honor it even if [`run_flow`](OidcService::run_flow) then fails.
pub struct ConsumedFlow {
    flow: OidcFlowState,
    /// The frontend URL to return the browser to, recovered before any fallible
    /// callback work so a failed flow still redirects there.
    redirect_uri: Option<String>,
}

impl ConsumedFlow {
    /// The caller's redirect target, if one was supplied at flow start.
    #[must_use]
    pub fn redirect_uri(&self) -> Option<&str> {
        self.redirect_uri.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service_with(origins: &[&str], schemes: &[&str]) -> OidcCore {
        OidcCore {
            http: reqwest::Client::new(),
            providers: Vec::new(),
            allowed_redirect_origins: origins.iter().map(ToString::to_string).collect(),
            allowed_redirect_schemes: schemes.iter().map(ToString::to_string).collect(),
        }
    }

    fn service_with_origins(origins: &[&str]) -> OidcCore {
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
