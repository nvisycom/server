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

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::AccountApiTokenRepository;
use nvisy_postgres::types::ApiTokenType;
use serde::{Deserialize, Serialize};

use crate::extract::{AuthState, Json, Path, Query, SecurityContext, ValidateJson};
use crate::handler::request::{DesktopTokenRequest, IdentityPathParams, OidcCallbackQuery};
use crate::handler::response::AccountDesktopToken;
use crate::response::{CookieConfig, ErrorKind, ErrorResponse, RedirectResult, Result, WebSession};
use crate::service::{
    AccountProvisioner, AuthIssuer, CallbackOutcome, OidcPurpose, OidcService, RedirectKind,
    ServiceState,
};

/// Tracing target for OIDC sign-in operations.
const TRACING_TARGET: &str = "nvisy_server::handler::auth_oidc";

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
    State(oidc): State<OidcService>,
    Path(path_params): Path<IdentityPathParams>,
    Query(query): Query<OidcStartQuery>,
) -> Result<(StatusCode, Json<OidcStartResponse>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting OIDC sign-in");
    let authorize_url = oidc
        .begin_flow(
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
        ErrorKind::Unauthorized.with_message("Re-authentication required to link a provider")
    })?;
    oidc.consume_reauth_proof(auth_state.account_id, proof)
        .await?;

    let authorize_url = oidc
        .begin_flow(
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
    State(oidc): State<OidcService>,
    auth_state: AuthState,
    Path(path_params): Path<IdentityPathParams>,
    Query(query): Query<OidcStartQuery>,
) -> Result<(StatusCode, Json<OidcStartResponse>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting OIDC step-up re-authentication");
    let authorize_url = oidc
        .begin_flow(
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
    State(provisioner): State<AccountProvisioner>,
    auth_state: AuthState,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<DesktopTokenRequest>,
) -> Result<(StatusCode, Json<AccountDesktopToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Minting desktop app token");

    // The target must be an allow-listed desktop scheme. Refuse a web origin (or
    // anything else) so this endpoint cannot be used to mint a token toward an
    // http page that would then hold a bearer credential.
    if oidc.classify_redirect(&request.redirect_uri) != Some(RedirectKind::DesktopScheme) {
        return Err(
            ErrorKind::BadRequest.with_message("redirectUri is not an allowed desktop scheme")
        );
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
            .with_message("Desktop tokens can only be minted from a browser session"));
    }

    let account = provisioner
        .load_active(&mut conn, auth_state.account_id)
        .await?;
    OidcService::gate_account_status(&account)?;

    let api_token = issuer
        .issue_app_token(&mut conn, &account, security)
        .await?;

    Ok((
        StatusCode::OK,
        Json(AccountDesktopToken {
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
        .response::<200, Json<AccountDesktopToken>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
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
/// stashed purpose selects the action). Transport only: it drives the
/// [`OidcService`] flow and turns its outcome into the browser redirect.
#[tracing::instrument(skip_all)]
async fn oidc_callback(
    State(oidc): State<OidcService>,
    State(cookie): State<CookieConfig>,
    security: SecurityContext,
    Query(query): Query<OidcCallbackQuery>,
) -> Response {
    tracing::debug!(target: TRACING_TARGET, "Completing OIDC callback");

    // Recover the flow first, so the caller's redirect target is known even when
    // the subsequent work fails — a failed sign-in still returns the browser to
    // the frontend with `signin=error` rather than a dead fallback page.
    let flow = match oidc.consume_flow(&query).await {
        Ok(flow) => flow,
        Err(err) => {
            // No flow means no trusted redirect target (an unknown/expired/replayed
            // state), so fall back to the in-page result.
            tracing::warn!(target: TRACING_TARGET, error = %err, "OIDC callback state invalid");
            return RedirectResult::Error.into_redirect(None);
        }
    };
    let redirect_uri = flow.redirect_uri().map(str::to_owned);

    match oidc.run_flow(security, flow, query).await {
        Ok(outcome) => {
            tracing::info!(target: TRACING_TARGET, kind = outcome.kind(), "OIDC callback succeeded");
            outcome_into_redirect(outcome, redirect_uri.as_deref(), cookie)
        }
        Err(err) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, "OIDC callback failed");
            RedirectResult::Error.into_redirect(redirect_uri.as_deref())
        }
    }
}

/// Builds the browser response returning to the frontend for a completed flow
/// outcome. Pure transport: it maps each [`CallbackOutcome`] to its delivery
/// mechanism (a cookie, a deep-link query, or a fragment). `cookie` supplies the
/// session-cookie policy for a web sign-in.
fn outcome_into_redirect(
    outcome: CallbackOutcome,
    redirect_uri: Option<&str>,
    cookie: CookieConfig,
) -> Response {
    match outcome {
        CallbackOutcome::SignedIn { jwt } => {
            // Web sign-in delivers the session as an HttpOnly cookie (plus its
            // CSRF cookie) set on the success redirect — never in the URL.
            let redirect = RedirectResult::Success.into_redirect(redirect_uri);
            (WebSession::new(jwt, cookie).into_jar(), redirect).into_response()
        }
        CallbackOutcome::DesktopSignedIn { jwt } => {
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
        CallbackOutcome::Linked => RedirectResult::Success.into_redirect(redirect_uri),
        CallbackOutcome::Reauthed { proof } => RedirectResult::Fragment {
            name: "reauthProof",
            value: &proof,
        }
        .into_redirect(redirect_uri),
    }
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
