//! OIDC flows (Google, Microsoft): sign-in, account linking, and step-up
//! re-authentication.
//!
//! Every flow is an OpenID Connect authorization-code exchange — a two-step,
//! browser-driven round-trip that all share one callback:
//!
//! 1. A `start` endpoint begins the authorization, stashes the CSRF state, PKCE
//!    verifier, nonce, the caller's redirect target, and the flow's *purpose* in
//!    a short-lived NATS KV entry, and returns the provider authorize URL.
//! 2. `GET /auth/{provider}/callback` — the provider redirects here with a `code`
//!    and the `state`. The stashed entry is consumed (single-use, atomically),
//!    the code is exchanged, the ID token is verified, and the stashed purpose
//!    selects the action:
//!    - **sign-in** — resolve the identity to an account (provisioning on first
//!      sign-in, or auto-linking to an existing account on a verified email) and
//!      mint a session, delivered by the redirect target: a **web** origin gets an
//!      `HttpOnly` session cookie; a **desktop** deep-link scheme gets a long-lived
//!      `app` token in the redirect's URL query (for the native app);
//!    - **link** — attach the verified provider identity to the authenticated
//!      account that started the flow (started under the account-identities
//!      resource; requires a step-up proof);
//!    - **reauth** — confirm current control of a provider already linked to the
//!      account and mint a single-use step-up proof.
//!
//! A separate `POST /auth/desktop/token` mints the same native-app token for a
//! *password* desktop login: the browser completes a normal cookie login, then the
//! frontend exchanges that session for an `app` token to hand to the app.
//!
//! The PKCE verifier and nonce stay server-side (round-tripping them through the
//! browser would defeat them), so the flow state is stored, single-use, and
//! TTL-expired. The callback conveys its result by redirect (only to an
//! allow-listed target): a web sign-in sets the session cookie on that redirect,
//! so no token appears in the URL. The step-up reauth proof rides in the web
//! redirect's fragment (not logged or refereed); a desktop sign-in carries its
//! `app` token in the custom-scheme deep-link's query (no server hop to leak to).

use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use nvisy_nats::NatsClient;
use nvisy_nats::kv::{
    OidcStateBucket as OidcStateKvBucket, OidcStateKey, ReauthProofBucket as ReauthProofKvBucket,
    ReauthProofKey,
};
use nvisy_postgres::model::{Account, NewAccount, NewAccountIdentity};
use nvisy_postgres::query::{
    AccountApiTokenRepository, AccountIdentityRepository, AccountRepository, LinkIdentityOutcome,
};
use nvisy_postgres::types::{ApiTokenType, HANDLE_MAX_LENGTH, Handle, IdentityProvider};
use nvisy_postgres::{AsyncConnection, Error as PgError, PgClient, PgConn};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{AuthState, Json, Path, Query, SecurityContext, ValidateJson};
use crate::handler::request::{DesktopTokenRequest, IdentityPathParams, OidcCallbackQuery};
use crate::handler::response::{DesktopToken, ErrorResponse};
use crate::handler::{ErrorKind, Result};
use crate::response::{CookieConfig, RedirectResult, WebSession};
use crate::service::{
    AuthIssuer, OidcAuthorization, OidcIdentity, OidcService, RedirectKind, ServiceState,
};

/// Tracing target for OIDC sign-in operations.
const TRACING_TARGET: &str = "nvisy_server::handler::auth_oidc";

/// How many suffixed handles to try when deriving a unique username on
/// provisioning, before giving up. A collision past this many is implausible
/// (each is a distinct suffix), so exhausting it is a server-side failure.
const MAX_USERNAME_ATTEMPTS: u32 = 100;

/// What an in-flight OIDC flow is for. All three share the same authorize +
/// callback machinery; only the callback's action differs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum OidcPurpose {
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

/// The stashed state of an in-flight OIDC flow, held between `start` and
/// `callback` in the [`OidcStateBucket`]. Keyed by the CSRF state token.
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

/// Consumes a step-up re-authentication proof: verifies it exists and is for
/// `account_id`, then deletes it (single-use). Returns an error if the proof is
/// missing, expired, already used, or for a different account.
///
/// Required by every credential-adding action so a merely-stolen session (with
/// no way to complete a fresh provider re-auth) cannot mint a durable credential.
pub(crate) async fn consume_reauth_proof(
    nats: &NatsClient,
    account_id: Uuid,
    proof: &str,
) -> Result<()> {
    let key = ReauthProofKey::from_str(proof)
        .map_err(|_| ErrorKind::Unauthorized.with_message("Invalid re-authentication proof"))?;
    let store = nats.kv_store::<ReauthProofBucket>().await?;
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

/// The response to a successful sign-in start: where to send the user.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcStartResponse {
    /// The provider authorize URL the client should redirect the user to.
    pub authorize_url: String,
}

/// Begins an OIDC sign-in and returns the provider authorize URL.
#[tracing::instrument(skip_all, fields(provider = ?path_params.provider))]
async fn start_sign_in(
    State(nats): State<NatsClient>,
    State(oidc): State<OidcService>,
    Path(path_params): Path<IdentityPathParams>,
    Query(query): Query<OidcStartQuery>,
) -> Result<(StatusCode, Json<OidcStartResponse>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting OIDC sign-in");
    let authorize_url = begin_flow(
        &nats,
        &oidc,
        path_params.provider,
        query.redirect_uri,
        OidcPurpose::SignIn,
    )
    .await?;
    Ok((StatusCode::OK, Json(OidcStartResponse { authorize_url })))
}

fn start_sign_in_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start OIDC sign-in")
        .description(
            "Begins an OpenID Connect sign-in with the given provider and returns the provider \
             authorize URL to redirect the user to. On consent, the provider redirects to the \
             callback, which signs the user in.",
        )
        .response::<200, Json<OidcStartResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<503, Json<ErrorResponse>>()
}

/// Begins an authenticated OIDC *link*: connects the given provider to the
/// caller's current account. Same authorization as sign-in, but the flow carries
/// the account id so the callback attaches the identity rather than signing in.
///
/// Mounted under the account-identities resource
/// (`POST /account/identities/{provider}`), not `/auth`, so linking and
/// unlinking a provider sit symmetrically on the same resource. The OIDC redirect
/// machinery lives here beside the shared callback.
#[tracing::instrument(skip_all, fields(provider = ?path_params.provider, account_id = %auth_state.account_id))]
pub(crate) async fn start_link(
    State(nats): State<NatsClient>,
    State(oidc): State<OidcService>,
    auth_state: AuthState,
    Path(path_params): Path<IdentityPathParams>,
    Query(query): Query<OidcStartQuery>,
) -> Result<(StatusCode, Json<OidcStartResponse>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting OIDC account link");

    // Linking a provider adds a credential, so it requires a fresh step-up proof
    // (from the reauth endpoint): a merely-stolen session must not be able to
    // attach a new sign-in method. The proof is consumed here, before the OIDC
    // round-trip begins.
    let proof = query.reauth_proof.as_deref().ok_or_else(|| {
        ErrorKind::Unauthorized
            .with_message("Re-authentication required to link a provider")
            .with_resource("account")
    })?;
    consume_reauth_proof(&nats, auth_state.account_id, proof).await?;

    let authorize_url = begin_flow(
        &nats,
        &oidc,
        path_params.provider,
        query.redirect_uri,
        OidcPurpose::Link {
            account_id: auth_state.account_id,
        },
    )
    .await?;
    Ok((StatusCode::OK, Json(OidcStartResponse { authorize_url })))
}

pub(crate) fn start_link_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Link a provider")
        .description(
            "Begins linking the given OpenID Connect provider to the authenticated account and \
             returns the provider authorize URL. Requires a step-up re-authentication proof \
             (`reauthProof`, from the reauth endpoint). On consent, the callback attaches the \
             verified provider identity to the caller's account.",
        )
        .response::<200, Json<OidcStartResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<503, Json<ErrorResponse>>()
}

/// Begins a step-up re-authentication: proves the caller currently controls a
/// provider identity already linked to their account, so a credential-adding
/// action (setting a first password, linking a new provider) can require more
/// than a merely-live session. The callback mints a single-use proof.
#[tracing::instrument(skip_all, fields(provider = ?path_params.provider, account_id = %auth_state.account_id))]
async fn start_reauth(
    State(nats): State<NatsClient>,
    State(oidc): State<OidcService>,
    auth_state: AuthState,
    Path(path_params): Path<IdentityPathParams>,
    Query(query): Query<OidcStartQuery>,
) -> Result<(StatusCode, Json<OidcStartResponse>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting OIDC step-up re-authentication");
    let authorize_url = begin_flow(
        &nats,
        &oidc,
        path_params.provider,
        query.redirect_uri,
        OidcPurpose::Reauth {
            account_id: auth_state.account_id,
        },
    )
    .await?;
    Ok((StatusCode::OK, Json(OidcStartResponse { authorize_url })))
}

fn start_reauth_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start OIDC step-up re-authentication")
        .description(
            "Begins a step-up re-authentication with a provider already linked to the \
             authenticated account, and returns the provider authorize URL. On consent, the \
             callback mints a short-lived, single-use proof required to add a credential \
             (set a first password, or link a new provider).",
        )
        .response::<200, Json<OidcStartResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<503, Json<ErrorResponse>>()
}

/// Mints a native-app (desktop) session token for the authenticated account.
///
/// The desktop app can't use the browser cookie session (its webview and its Rust
/// HTTP client have separate cookie jars, and an `HttpOnly` cookie is invisible to
/// the app). Instead, after the user completes a normal browser login, the
/// frontend calls this with the desktop deep-link; it mints a long-lived `app`
/// token and returns it for the frontend to hand back to the app via the
/// deep-link. Authenticated by the just-established session, so a caller can only
/// mint a token for their own account. No cookie is set on the response.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn mint_desktop_token(
    State(pg_client): State<PgClient>,
    State(oidc): State<OidcService>,
    State(issuer): State<AuthIssuer>,
    auth_state: AuthState,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<DesktopTokenRequest>,
) -> Result<(StatusCode, Json<DesktopToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Minting desktop app token");

    // The target must be an allow-listed desktop scheme. Refuse a web origin (or
    // anything else) so this endpoint cannot be used to mint a token toward an
    // http page that would then hold a bearer credential.
    if oidc.classify_redirect(&request.redirect_uri) != Some(RedirectKind::DesktopScheme) {
        return Err(ErrorKind::BadRequest
            .with_message("redirectUri is not an allowed desktop scheme")
            .with_resource("account"));
    }

    let mut conn = pg_client.get_connection().await?;

    // Only a browser (`web`) session may mint a desktop token: the desktop login
    // completes in the browser first. Rejecting `app`/`api` tokens stops a leaked
    // long-lived `app` token from renewing itself indefinitely by minting fresh
    // `app` tokens.
    let session = conn
        .find_account_api_token_by_id(auth_state.token_id)
        .await?
        .ok_or_else(|| ErrorKind::Unauthorized.with_message("Session not found"))?;
    if session.session_type != ApiTokenType::Web {
        return Err(ErrorKind::Forbidden
            .with_message("Desktop tokens can only be minted from a browser session")
            .with_resource("session"));
    }

    let account = load_active_account(&mut conn, auth_state.account_id).await?;
    gate_account_status(&account)?;

    let api_token = issuer
        .issue_app_token(&mut conn, &account, security)
        .await?;

    Ok((
        StatusCode::OK,
        Json(DesktopToken {
            api_token,
            redirect_uri: request.redirect_uri,
        }),
    ))
}

fn mint_desktop_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Mint a desktop app token")
        .description(
            "Mints a long-lived native-app session token for the authenticated account, to be \
             delivered to the desktop app via the given desktop deep-link. Requires an active \
             browser session (the desktop login completes in the browser first). The `redirectUri` \
             must be a configured desktop scheme.",
        )
        .response::<200, Json<DesktopToken>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Begins an authorization (sign-in, link, or reauth) and stashes the flow
/// state, returning the provider authorize URL.
async fn begin_flow(
    nats: &NatsClient,
    oidc: &OidcService,
    provider: IdentityProvider,
    redirect_uri: Option<String>,
    purpose: OidcPurpose,
) -> Result<String> {
    // Reject a redirect target that is not an allow-listed frontend origin before
    // starting the flow: the callback carries the minted session token (or a
    // reauth proof), so it must never be sent to a caller-chosen host.
    if let Some(redirect_uri) = &redirect_uri
        && !oidc.is_redirect_allowed(redirect_uri)
    {
        return Err(ErrorKind::BadRequest
            .with_message("redirectUri is not an allowed origin")
            .with_resource("account"));
    }

    let OidcAuthorization {
        authorize_url,
        csrf_state,
        pkce_verifier,
        nonce,
    } = oidc.begin(provider).await?;

    let flow = OidcFlowState {
        provider,
        pkce_verifier,
        nonce,
        redirect_uri,
        purpose,
    };
    let store = nats.kv_store::<OidcStateBucket>().await?;
    store.put(&OidcStateKey(csrf_state), &flow).await?;

    Ok(authorize_url)
}

/// Optional query parameters for starting a sign-in, link, or reauth.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OidcStartQuery {
    /// Frontend URL to return to once done. Carried through the flow and used to
    /// build the callback's redirect.
    redirect_uri: Option<String>,
    /// A step-up re-authentication proof. Required to start a *link* (adding a
    /// provider is a credential-adding action, so a live session alone is not
    /// enough); ignored for sign-in and reauth.
    reauth_proof: Option<String>,
}

/// Completes an OIDC flow: the provider redirects here with the code and state.
/// Serves sign-in, account-link, and step-up re-authentication (the flow's
/// stashed purpose selects the action).
#[tracing::instrument(skip_all)]
async fn oidc_callback(
    State(pg_client): State<PgClient>,
    State(nats): State<NatsClient>,
    State(oidc): State<OidcService>,
    State(issuer): State<AuthIssuer>,
    State(cookie): State<CookieConfig>,
    security: SecurityContext,
    Query(query): Query<OidcCallbackQuery>,
) -> Response {
    tracing::debug!(target: TRACING_TARGET, "Completing OIDC callback");

    // Recover the flow first, so the caller's redirect target is known even when
    // the subsequent work fails — a failed sign-in still returns the browser to
    // the frontend with `signin=error` rather than a dead fallback page.
    let flow = match consume_flow(&nats, &query).await {
        Ok(flow) => flow,
        Err(err) => {
            // No flow means no trusted redirect target (an unknown/expired/replayed
            // state), so fall back to the in-page result.
            tracing::warn!(target: TRACING_TARGET, error = %err, "OIDC callback state invalid");
            return RedirectResult::Error.into_redirect(None);
        }
    };
    let redirect_uri = flow.redirect_uri.clone();

    match run_flow(&pg_client, &oidc, &nats, &issuer, security, flow, query).await {
        Ok(outcome) => {
            tracing::info!(target: TRACING_TARGET, kind = outcome.kind(), "OIDC callback succeeded");
            outcome.into_redirect(redirect_uri.as_deref(), cookie)
        }
        Err(err) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, "OIDC callback failed");
            RedirectResult::Error.into_redirect(redirect_uri.as_deref())
        }
    }
}

/// The result of a completed callback, by flow purpose.
enum CallbackOutcome {
    /// A web sign-in: the minted session JWT, delivered to the browser as an
    /// HttpOnly session cookie set on the callback redirect.
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
    fn kind(&self) -> &'static str {
        match self {
            Self::SignedIn { .. } => "sign_in",
            Self::DesktopSignedIn { .. } => "sign_in_desktop",
            Self::Linked => "link",
            Self::Reauthed { .. } => "reauth",
        }
    }

    /// Builds the browser response returning to the frontend for this outcome.
    /// `cookie` supplies the session-cookie policy for a web sign-in.
    fn into_redirect(self, redirect_uri: Option<&str>, cookie: CookieConfig) -> Response {
        match self {
            Self::SignedIn { jwt } => {
                // Web sign-in delivers the session as an HttpOnly cookie (plus its
                // CSRF cookie) set on the success redirect — never in the URL.
                let redirect = RedirectResult::Success.into_redirect(redirect_uri);
                (WebSession::new(jwt, cookie).into_jar(), redirect).into_response()
            }
            Self::DesktopSignedIn { jwt } => {
                // Desktop sign-in hands the app token back in the deep-link's URL
                // query (`?token=…`) — never a cookie the app's webview can't see.
                // The target is the allow-listed custom scheme, which has no server
                // hop, so the query is safe and matches the native OAuth convention.
                RedirectResult::Query {
                    name: "token",
                    value: &jwt,
                }
                .into_redirect(redirect_uri)
            }
            Self::Linked => RedirectResult::Success.into_redirect(redirect_uri),
            Self::Reauthed { proof } => RedirectResult::Fragment {
                name: "reauthProof",
                value: &proof,
            }
            .into_redirect(redirect_uri),
        }
    }
}

/// Consumes the pending flow state single-use (atomically) and returns it. Split
/// from the work below so the caller recovers the flow's `redirect_uri` before
/// any fallible step, and can honor it even when that step fails.
async fn consume_flow(nats: &NatsClient, query: &OidcCallbackQuery) -> Result<OidcFlowState> {
    // Validate the state before touching the store so a malformed value maps to a
    // clean BadRequest rather than a KV error.
    let key = OidcStateKey::from_str(&query.state)
        .map_err(|_| ErrorKind::BadRequest.with_message("Invalid authorization state"))?;

    // `take` consumes single-use and atomically, so two concurrent callbacks
    // carrying the same state cannot both proceed. A missing entry means an
    // unknown, expired, or already-consumed state.
    let store = nats.kv_store::<OidcStateBucket>().await?;
    store
        .take(&key)
        .await?
        .ok_or_else(|| ErrorKind::BadRequest.with_message("Invalid or expired authorization"))
}

/// Runs a consumed flow's work: exchange and verify the code, then perform the
/// flow's purpose (sign in, link, or mint a reauth proof). The flow state has
/// already been consumed by [`consume_flow`], so its `redirect_uri` is the
/// caller's and is applied by the callback whether this succeeds or fails.
async fn run_flow(
    pg_client: &PgClient,
    oidc: &OidcService,
    nats: &NatsClient,
    issuer: &AuthIssuer,
    security: SecurityContext,
    flow: OidcFlowState,
    query: OidcCallbackQuery,
) -> Result<CallbackOutcome> {
    // A denial (or any provider error) arrives with `error` and no `code`.
    if let Some(error) = query.error {
        return Err(ErrorKind::Unauthorized
            .with_message("Authorization was denied")
            .with_context(error));
    }
    let code = query
        .code
        .ok_or_else(|| ErrorKind::BadRequest.with_message("Authorization callback missing code"))?;

    // Exchange the code and verify the ID token (signature, aud, iss, exp, nonce).
    let identity = oidc
        .complete(flow.provider, code, flow.pkce_verifier, flow.nonce)
        .await?;

    let mut conn = pg_client.get_connection().await?;

    match flow.purpose {
        OidcPurpose::SignIn => {
            let account = resolve_account(&mut conn, flow.provider, identity).await?;
            gate_account_status(&account)?;

            // The redirect target decides how the session is delivered. A desktop
            // deep-link scheme gets a long-lived `app` token in the callback's URL
            // query; a web origin gets an HttpOnly session cookie. The target was
            // already allow-listed at flow start; a `None` here means it is neither
            // kind (which `begin_flow` would have rejected), so default to the web
            // cookie path.
            let kind = flow
                .redirect_uri
                .as_deref()
                .and_then(|uri| oidc.classify_redirect(uri));

            if kind == Some(RedirectKind::DesktopScheme) {
                let jwt = issuer
                    .issue_app_token(&mut conn, &account, security)
                    .await?;
                Ok(CallbackOutcome::DesktopSignedIn { jwt })
            } else {
                // OIDC web sign-in delivers a remembered browser session as an
                // HttpOnly cookie set on the callback redirect — the token never
                // appears in the URL.
                let jwt = issuer
                    .issue_web_session(&mut conn, &account, true, security)
                    .await?;
                Ok(CallbackOutcome::SignedIn { jwt })
            }
        }
        OidcPurpose::Link { account_id } => {
            link_account(&mut conn, account_id, flow.provider, identity).await?;
            Ok(CallbackOutcome::Linked)
        }
        OidcPurpose::Reauth { account_id } => {
            // The verified identity must belong to the account being re-authed:
            // proving control of *some* provider account is not enough, it must be
            // one linked here.
            let matches = conn
                .find_identity_by_subject(flow.provider, &identity.subject)
                .await?
                .is_some_and(|linked| linked.account_id == account_id);
            if !matches {
                return Err(ErrorKind::Unauthorized
                    .with_message("Re-authentication did not match a linked identity")
                    .with_resource("account"));
            }
            let proof = mint_reauth_proof(nats, account_id).await?;
            Ok(CallbackOutcome::Reauthed { proof })
        }
    }
}

/// Refuses a sign-in for a suspended or deleted account, before any session token
/// is minted — mirroring what password login gates on.
fn gate_account_status(account: &Account) -> Result<()> {
    if account.is_suspended() {
        return Err(ErrorKind::Forbidden
            .with_message("Account is suspended")
            .with_resource("account"));
    }
    if account.is_deleted() {
        return Err(ErrorKind::Forbidden
            .with_message("Account has been deleted")
            .with_resource("account"));
    }
    Ok(())
}

/// Mints and stores a single-use step-up proof for `account_id`, returning its
/// opaque key. The proof is short-lived (the bucket's TTL) and consumed by the
/// credential-adding action.
async fn mint_reauth_proof(nats: &NatsClient, account_id: Uuid) -> Result<String> {
    let proof = generate_proof_token();
    let store = nats.kv_store::<ReauthProofBucket>().await?;
    store
        .put(&ReauthProofKey(proof.clone()), &ReauthProof { account_id })
        .await?;
    Ok(proof)
}

/// Links an OIDC identity to an existing account, mapping the repository's
/// race-tolerant [`LinkIdentityOutcome`] to the handler result: a successful or
/// already-present link is `Ok`, and a provider slot already taken by a
/// *different* account is a clean 409 rather than a 500.
async fn link_oidc_identity(conn: &mut PgConn, identity: NewAccountIdentity) -> Result<()> {
    match conn.link_oidc_identity(identity).await? {
        LinkIdentityOutcome::Linked | LinkIdentityOutcome::AlreadyLinked => Ok(()),
        LinkIdentityOutcome::ProviderConflict => Err(ErrorKind::Conflict
            .with_message("An account already uses a different provider account")
            .with_resource("account_identity")),
    }
}

/// Resolves the account for a verified OIDC identity, in order of preference:
///
/// 1. **Returning user** — an identity already exists for this `(provider,
///    subject)`; reuse its account.
/// 2. **Link to an existing account** — the provider asserts a *verified* email
///    that matches an account (e.g. one created by password signup); attach a new
///    OIDC identity to it, so the two sign-in methods share one account.
/// 3. **Provision** — otherwise create a new account and its OIDC identity.
///
/// Linking requires a verified email: an unverified address could be one the
/// signer does not control, so linking on it would let an attacker attach their
/// provider identity to someone else's account.
async fn resolve_account(
    conn: &mut PgConn,
    provider: IdentityProvider,
    identity: OidcIdentity,
) -> Result<Account> {
    // 1. Returning user: an identity for this subject already exists.
    if let Some(existing) = conn
        .find_identity_by_subject(provider, &identity.subject)
        .await?
        && let Some(account) = conn.find_account_by_id(existing.account_id).await?
    {
        return Ok(account);
    }

    // A new identity needs the provider-asserted email: to provision, it becomes
    // the account's required primary address; to link, it is the match key.
    let email = identity.email.ok_or_else(|| {
        ErrorKind::BadRequest
            .with_message("Sign-in provider did not return an email address")
            .with_resource("account")
    })?;

    // 2. An account already uses this email.
    if let Some(account) = conn.find_account_by_email(&email).await? {
        // Link only when the provider verified the email: an unverified address
        // could be one the signer does not control, and linking on it would let
        // them attach their provider identity to someone else's account.
        if !identity.email_verified {
            tracing::warn!(
                target: TRACING_TARGET,
                account_id = %account.id,
                provider = ?provider,
                "Refusing to link OIDC identity: provider did not verify the email",
            );
            return Err(ErrorKind::Conflict
                .with_message(
                    "An account already uses this email; sign in with your existing method \
                     or verify the email with the provider first",
                )
                .with_resource("account"));
        }

        // The matched account may already have a *different* identity for this
        // provider (a different subject). Only one identity per provider is
        // allowed, so linking would trip the unique index; surface a clean
        // conflict instead of a 500.
        if conn
            .find_account_identity(account.id, provider)
            .await?
            .is_some()
        {
            tracing::warn!(
                target: TRACING_TARGET,
                account_id = %account.id,
                provider = ?provider,
                "Refusing to link OIDC identity: account already has one for this provider",
            );
            return Err(ErrorKind::Conflict
                .with_message(
                    "An account already uses this email with a different provider account",
                )
                .with_resource("account"));
        }

        link_oidc_identity(
            conn,
            NewAccountIdentity::oidc(account.id, provider, identity.subject, Some(email)),
        )
        .await?;
        tracing::info!(
            target: TRACING_TARGET,
            account_id = %account.id,
            provider = ?provider,
            "Linked OIDC identity to existing account",
        );
        return Ok(account);
    }

    // 3. Provision a new account and its OIDC identity together, so an account
    // never exists without a way to authenticate.
    //
    // Only provision on a verified email: the address becomes the new account's
    // primary (and its future match key for step 2), so an unverified one could
    // seed an account under an address the signer does not control.
    if !identity.email_verified {
        tracing::warn!(
            target: TRACING_TARGET,
            provider = ?provider,
            "Refusing to provision account: provider did not verify the email",
        );
        return Err(ErrorKind::BadRequest
            .with_message(
                "Sign-in provider did not verify your email address; verify it with the \
                 provider and try again",
            )
            .with_resource("account"));
    }

    let username = derive_unique_username(conn, &email).await?;
    let new_account = NewAccount {
        username,
        display_name: None,
        email_address: email.clone(),
        avatar_url: None,
        timezone: None,
        locale: None,
    };

    let account = conn
        .transaction(async |conn| {
            let account = conn.create_account(new_account).await?;
            conn.create_account_identity(NewAccountIdentity::oidc(
                account.id,
                provider,
                identity.subject,
                Some(email),
            ))
            .await?;
            Ok::<_, PgError>(account)
        })
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account.id,
        provider = ?provider,
        "Provisioned account from OIDC sign-in",
    );

    Ok(account)
}

/// Attaches a verified OIDC identity to an already-authenticated account (the
/// account that started an authenticated link flow).
///
/// Idempotent for the same account: re-linking an identity already on this
/// account is a no-op. Refuses to move an identity already linked to a *different*
/// account (its provider subject is unique), so one provider login cannot be
/// hijacked onto another account.
async fn link_account(
    conn: &mut PgConn,
    account_id: Uuid,
    provider: IdentityProvider,
    identity: OidcIdentity,
) -> Result<Account> {
    if let Some(existing) = conn
        .find_identity_by_subject(provider, &identity.subject)
        .await?
    {
        if existing.account_id == account_id {
            // Already linked to this account: nothing to do.
            return load_active_account(conn, account_id).await;
        }
        tracing::warn!(
            target: TRACING_TARGET,
            account_id = %account_id,
            provider = ?provider,
            "Refusing to link an identity already linked to another account",
        );
        return Err(ErrorKind::Conflict
            .with_message("This provider identity is already linked to another account")
            .with_resource("account_identity"));
    }

    link_oidc_identity(
        conn,
        NewAccountIdentity::oidc(account_id, provider, identity.subject, identity.email),
    )
    .await?;
    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account_id,
        provider = ?provider,
        "Linked OIDC identity to the authenticated account",
    );

    load_active_account(conn, account_id).await
}

/// Loads a live account by id, or a not-found error (e.g. the account was
/// deleted between starting a link flow and its callback).
async fn load_active_account(conn: &mut PgConn, account_id: Uuid) -> Result<Account> {
    conn.find_account_by_id(account_id).await?.ok_or_else(|| {
        ErrorKind::NotFound
            .with_message("Account not found")
            .with_resource("account")
    })
}

/// Derives a unique username for a provisioned account from its email local
/// part, appending a numeric suffix on collision.
async fn derive_unique_username(conn: &mut PgConn, email: &str) -> Result<Handle> {
    let local_part = email.split('@').next().unwrap_or(email);
    // A local part may not slugify to a valid handle (too short, no usable
    // characters); fall back to a stable generated base so provisioning still
    // succeeds.
    let base = Handle::derive(local_part).unwrap_or_else(|| {
        Handle::derive(&format!("user-{}", Uuid::now_v7().simple()))
            .expect("a uuid-based handle is always valid")
    });

    if !conn.username_exists(&base).await? {
        return Ok(base);
    }
    // Widest suffix this loop can append, so we reserve room for the largest
    // `-{suffix}` up front. Without this, a `base` already at the length limit
    // would have its suffix truncated straight back off, and every candidate
    // would collapse to `base` and collide forever.
    let widest_suffix = MAX_USERNAME_ATTEMPTS.to_string().len();
    let reserved = HANDLE_MAX_LENGTH.saturating_sub(1 + widest_suffix);
    let stem = truncate_on_char_boundary(base.as_str(), reserved);
    for suffix in 1..=MAX_USERNAME_ATTEMPTS {
        // The stem already leaves room for the separator and suffix, so the
        // re-derive validates the combined form without truncating the suffix away.
        let candidate_text = format!("{stem}-{suffix}");
        if let Some(candidate) = Handle::derive(&candidate_text)
            && !conn.username_exists(&candidate).await?
        {
            return Ok(candidate);
        }
    }

    Err(ErrorKind::InternalServerError
        .with_message("Could not allocate a username for the new account")
        .with_resource("account"))
}

/// Truncates `value` to at most `max` bytes without splitting a UTF-8 character.
/// A derived [`Handle`] is ASCII, so `max` bytes equal `max` characters here.
fn truncate_on_char_boundary(value: &str, max: usize) -> &str {
    if value.len() <= max {
        return value;
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

/// Returns the public OIDC sign-in routes: sign-in start and the provider
/// callback.
///
/// Both are unauthenticated: sign-in is how a caller obtains a session in the
/// first place, and the provider's browser redirect lands on the callback with no
/// `Authorization` header. The callback serves the sign-in, link, and reauth flows
/// (they share one redirect URI); its security rests on the single-use,
/// unguessable CSRF state it consumes and the ID-token verification, and a link
/// flow additionally carries the authenticated account id captured at start. A
/// sign-in sets the session cookie on the callback redirect, so there is no
/// separate exchange step.
pub fn public_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        // The start endpoint is part of the API contract (a client initiates it),
        // so it is an `api_route` and appears in the OpenAPI spec.
        .api_route(
            "/auth/{provider}/start/",
            get_with(start_sign_in, start_sign_in_docs),
        )
        // The callback is a provider-driven browser redirect, never an SDK call,
        // so it is a plain route absent from the OpenAPI spec. It serves the
        // sign-in, link, and reauth flows (the stashed purpose selects the
        // action).
        .route("/auth/{provider}/callback/", get(oidc_callback))
        .with_path_items(|item| item.tag("Authentication"))
}

/// The authenticated OIDC routes: step-up re-authentication.
///
/// Reauth requires a session (the caller proves control of an identity already on
/// *their* account), so it lives among the private routes. Linking a provider is
/// mounted on the account-identities resource (see [`start_link`], wired by the
/// identities handler) for symmetry with unlinking. The callback is shared with
/// sign-in and remains public (see [`public_routes`]).
pub fn private_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/auth/{provider}/reauth/",
            get_with(start_reauth, start_reauth_docs),
        )
        // Desktop token minting: the browser session is exchanged for a native-app
        // Bearer token. A static path alongside the `{provider}` routes above; they
        // diverge after the second segment, so there is no route conflict.
        .api_route(
            "/auth/desktop/token/",
            post_with(mint_desktop_token, mint_desktop_token_docs),
        )
        .with_path_items(|item| item.tag("Authentication"))
}
