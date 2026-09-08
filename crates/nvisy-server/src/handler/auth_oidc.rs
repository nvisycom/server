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
//!      mint a session;
//!    - **link** — attach the verified provider identity to the authenticated
//!      account that started the flow (started under the account-identities
//!      resource; requires a step-up proof);
//!    - **reauth** — confirm current control of a provider already linked to the
//!      account and mint a single-use step-up proof.
//!
//! A signed-in session is the same [`AuthToken`] password login returns, so
//! everything downstream sees one session model. The PKCE verifier and nonce stay
//! server-side (round-tripping them through the browser would defeat them), so the
//! flow state is stored, single-use, and TTL-expired. The callback conveys its
//! result to the frontend by redirect (only to an allow-listed origin); any token
//! it hands back rides in the URL fragment, never the query.

use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum_extra::headers::UserAgent;
use jiff::{Span, Timestamp};
use nvisy_nats::NatsClient;
use nvisy_nats::kv::{
    OidcStateBucket as OidcStateKvBucket, OidcStateKey, ReauthProofBucket as ReauthProofKvBucket,
    ReauthProofKey,
};
use nvisy_postgres::model::{Account, NewAccount, NewAccountApiToken, NewAccountIdentity};
use nvisy_postgres::query::{
    AccountApiTokenRepository, AccountIdentityRepository, AccountRepository, LinkIdentityOutcome,
};
use nvisy_postgres::types::{ApiTokenType, HANDLE_MAX_LENGTH, Handle, IdentityProvider};
use nvisy_postgres::{AsyncConnection, Error as PgError, PgClient, PgConn};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::authentication::create_auth_header;
use crate::extract::{AuthState, Json, Path, Query, TypedHeader};
use crate::handler::request::{IdentityPathParams, OidcCallbackQuery};
use crate::handler::response::ErrorResponse;
use crate::handler::{ErrorKind, Result};
use crate::service::{
    OidcAuthorization, OidcIdentity, OidcService, ServiceState, SessionKeys, UserAgentParser,
};

/// Tracing target for OIDC sign-in operations.
const TRACING_TARGET: &str = "nvisy_server::handler::auth_oidc";

/// Session lifetime for an OIDC sign-in, in days, matching password login.
const SESSION_DAYS: i64 = 90;

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
#[tracing::instrument(skip_all, fields(provider = ?path_params.provider, account_id = %auth_claims.account_id))]
pub(crate) async fn start_link(
    State(nats): State<NatsClient>,
    State(oidc): State<OidcService>,
    AuthState(auth_claims): AuthState,
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
    consume_reauth_proof(&nats, auth_claims.account_id, proof).await?;

    let authorize_url = begin_flow(
        &nats,
        &oidc,
        path_params.provider,
        query.redirect_uri,
        OidcPurpose::Link {
            account_id: auth_claims.account_id,
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
#[tracing::instrument(skip_all, fields(provider = ?path_params.provider, account_id = %auth_claims.account_id))]
async fn start_reauth(
    State(nats): State<NatsClient>,
    State(oidc): State<OidcService>,
    AuthState(auth_claims): AuthState,
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
            account_id: auth_claims.account_id,
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
    State(auth_keys): State<SessionKeys>,
    State(ua_parser): State<UserAgentParser>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    Query(query): Query<OidcCallbackQuery>,
) -> Response {
    tracing::debug!(target: TRACING_TARGET, "Completing OIDC callback");

    let user_agent = user_agent.to_string();

    // Recover the flow first, so the caller's redirect target is known even when
    // the subsequent work fails — a failed sign-in still returns the browser to
    // the frontend with `signin=error` rather than a dead fallback page.
    let flow = match consume_flow(&nats, &query).await {
        Ok(flow) => flow,
        Err(err) => {
            // No flow means no trusted redirect target (an unknown/expired/replayed
            // state), so fall back to the in-page result.
            tracing::warn!(target: TRACING_TARGET, error = %err, "OIDC callback state invalid");
            return redirect_to_frontend(None, RedirectResult::Error);
        }
    };
    let redirect_uri = flow.redirect_uri.clone();

    match run_flow(
        &pg_client, &oidc, &nats, &auth_keys, &ua_parser, user_agent, flow, query,
    )
    .await
    {
        Ok(outcome) => {
            tracing::info!(target: TRACING_TARGET, kind = outcome.kind(), "OIDC callback succeeded");
            outcome.into_redirect(redirect_uri.as_deref())
        }
        Err(err) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, "OIDC callback failed");
            redirect_to_frontend(redirect_uri.as_deref(), RedirectResult::Error)
        }
    }
}

/// The result of a completed callback, by flow purpose.
enum CallbackOutcome {
    /// A sign-in: the minted session token is handed back to the frontend.
    SignedIn { api_token: String },
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
            Self::Linked => "link",
            Self::Reauthed { .. } => "reauth",
        }
    }

    /// Builds the browser response returning to the frontend for this outcome.
    fn into_redirect(self, redirect_uri: Option<&str>) -> Response {
        match self {
            Self::SignedIn { api_token } => redirect_to_frontend(
                redirect_uri,
                RedirectResult::Token {
                    name: "token",
                    value: &api_token,
                },
            ),
            Self::Linked => redirect_to_frontend(redirect_uri, RedirectResult::Success),
            Self::Reauthed { proof } => redirect_to_frontend(
                redirect_uri,
                RedirectResult::Token {
                    name: "reauthProof",
                    value: &proof,
                },
            ),
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
#[allow(clippy::too_many_arguments)]
async fn run_flow(
    pg_client: &PgClient,
    oidc: &OidcService,
    nats: &NatsClient,
    auth_keys: &SessionKeys,
    ua_parser: &UserAgentParser,
    user_agent: String,
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
            let api_token =
                mint_session(&mut conn, auth_keys, ua_parser, &account, user_agent).await?;
            Ok(CallbackOutcome::SignedIn { api_token })
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

/// Mints a session token for a signed-in account, mirroring password login. The
/// account's status is gated first, as password login does.
async fn mint_session(
    conn: &mut PgConn,
    auth_keys: &SessionKeys,
    ua_parser: &UserAgentParser,
    account: &Account,
    user_agent: String,
) -> Result<String> {
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

    let expired_at = Timestamp::now() + Span::new().days(SESSION_DAYS);
    let new_token = NewAccountApiToken {
        account_id: account.id,
        display_name: ua_parser.parse(&user_agent),
        ip_address: None,
        user_agent: Some(user_agent),
        is_remembered: Some(false),
        session_type: Some(ApiTokenType::Web),
        expired_at: Some(expired_at.into()),
    };
    let token = conn.create_account_api_token(new_token).await?;
    let auth_header = create_auth_header(auth_keys.clone(), account, &token)?;
    auth_header.into_string()
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

/// The outcome conveyed to the frontend by the callback redirect.
enum RedirectResult<'a> {
    /// A plain success with no value (a completed link).
    Success,
    /// A failure.
    Error,
    /// A success carrying a named token to hand to the frontend (a session token
    /// for sign-in, or a reauth proof for step-up).
    Token { name: &'a str, value: &'a str },
}

/// Returns the browser to the frontend with the callback outcome.
///
/// The `signin=success|error` status goes in the query string. A token (the
/// session token, or a reauth proof) instead goes in the URL **fragment**
/// (`#{name}={value}`): a fragment is not sent in the `Referer` header, not
/// shared when the URL is copied to logs or history sync, and stays client-side
/// for the frontend's script to read — so a bearer credential never rides in a
/// place that leaks it.
///
/// `base` is only ever an allow-listed origin (the redirect target is validated
/// when the flow starts). When no frontend URL is configured, or the configured
/// one somehow fails to parse, this renders a minimal self-describing page
/// instead of redirecting: a `data:` URL cannot be used, since browsers block
/// top-level navigation to it, and a token must never be placed in one regardless.
fn redirect_to_frontend(base: Option<&str>, result: RedirectResult<'_>) -> Response {
    let (status, token) = match result {
        RedirectResult::Success => ("success", None),
        RedirectResult::Error => ("error", None),
        RedirectResult::Token { name, value } => ("success", Some((name, value))),
    };

    // Build the redirect target through the URL parser so the query and fragment
    // are assembled and encoded correctly, rather than by string concatenation
    // that could mishandle an existing query or fragment on the base.
    if let Some(base) = base
        && let Ok(mut url) = url::Url::parse(base)
    {
        url.query_pairs_mut().append_pair("signin", status);
        match token {
            // The token goes in the fragment, never the query, so it is not
            // leaked via Referer, history, or logs. `Url` percent-encodes the
            // fragment it is given.
            Some((name, value)) => url.set_fragment(Some(&format!("{name}={value}"))),
            None => url.set_fragment(None),
        }
        return Redirect::to(url.as_str()).into_response();
    }

    // No usable frontend origin: render a minimal in-page result. Never a token —
    // the only outcomes reaching here carry none, since a token flow requires an
    // allow-listed redirect to have been validated at start.
    let body = format!("Sign-in {status}. You can close this window.");
    (StatusCode::OK, body).into_response()
}

/// Returns the public OIDC sign-in routes.
///
/// The public OIDC routes: sign-in start and the provider callback.
///
/// Both are unauthenticated: sign-in is how a caller obtains a session in the
/// first place, and the provider's browser redirect lands on the callback with
/// no `Authorization` header. The callback serves both the sign-in and link
/// flows (they share one redirect URI); its security rests on the single-use,
/// unguessable CSRF state it consumes and the ID-token verification, and a link
/// flow additionally carries the authenticated account id captured at start.
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
        .with_path_items(|item| item.tag("Authentication"))
}
