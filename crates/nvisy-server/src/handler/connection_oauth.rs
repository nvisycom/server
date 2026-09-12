//! OAuth authorization flow for cloud file-service connections.
//!
//! A cloud file provider (Google Drive, ...) is connected over OAuth2, not with
//! static credentials, so it cannot be created through the ordinary
//! [`create_connection`](super::connections) endpoint. Instead:
//!
//! 1. `POST .../connections/oauth/{provider}/start` — authorizes the caller,
//!    stashes the CSRF state and PKCE verifier (with the target workspace and the
//!    connection details) in a short-lived NATS KV entry, and returns the
//!    provider authorize URL to send the user to.
//! 2. `GET  .../connections/oauth/callback` — the provider redirects here with a
//!    `code` and the `state`. The stashed entry is consumed (single-use),
//!    the code is exchanged for tokens, and the connection is created in the
//!    stashed workspace.
//!
//! The PKCE verifier must stay server-side (putting it in the round-tripped
//! `state` would defeat PKCE), so the flow state is stored rather than encoded in
//! the redirect. It is ephemeral, single-use, and TTL-expired.

use std::str::FromStr;

use aide::axum::ApiRouter;
use aide::axum::routing::post_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use nvisy_file_service::FileService;
use nvisy_file_service::provider::{ConnectionSettings, FileServiceConfig, FileServiceProvider};
use nvisy_nats::NatsClient;
use nvisy_nats::kv::{OAuthStateBucket as OAuthStateKvBucket, OAuthStateKey};
use nvisy_postgres::model::NewWorkspaceConnection;
use nvisy_postgres::query::WorkspaceConnectionRepository;
use nvisy_postgres::{AsyncConnection, PgClient};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{OAuthCallbackQuery, OAuthStartPathParams, StartFileServiceOAuth};
use crate::response::{Error, ErrorKind, ErrorResponse, Result, connection_result_redirect};
use crate::service::event::EventEmitter;
use crate::service::{ConnectionConfig, CryptoService, FileServiceRedirect, ServiceState, event};

/// Tracing target for connection OAuth operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connection_oauth";

/// The stashed state of an in-flight OAuth authorization, held between `start`
/// and `callback` in the [`OAuthStateBucket`]. Keyed by the CSRF state token.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OAuthFlowState {
    /// Workspace the connection will be created in.
    workspace_id: Uuid,
    /// The workspace's slug, carried so the post-auth redirect can substitute a
    /// `{workspaceSlug}` placeholder without a lookup in the callback.
    workspace_slug: String,
    /// Account that started the flow; the connection is attributed to it.
    account_id: Uuid,
    /// The provider being connected.
    provider: FileServiceProvider,
    /// Display name for the connection to create.
    display_name: String,
    /// Optional sync root (folder id or path) to scope the sync to.
    root: Option<String>,
    /// The PKCE verifier to present when exchanging the code.
    pkce_verifier: String,
}

/// The OAuth-state bucket pinned to this server's flow-state value. The bucket's
/// static config (name, TTL, key) lives in the NATS layer; this alias fixes the
/// value type once so call sites need only name the bucket.
type OAuthStateBucket = OAuthStateKvBucket<OAuthFlowState>;

/// The result of a completed OAuth callback: the connection that was created and
/// the workspace it belongs to (used to build the post-auth redirect).
struct CallbackOutcome {
    connection_id: Uuid,
    workspace_slug: String,
}

/// The response to a successful authorization start: where to send the user.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartResponse {
    /// The provider authorize URL the client should redirect the user to.
    pub authorize_url: String,
}

/// Starts a cloud file-service OAuth authorization.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        provider = ?path_params.provider,
    )
)]
async fn start_oauth(
    State(nats): State<NatsClient>,
    State(cloud): State<FileService>,
    authz: Authorized<markers::ManageConnections>,
    Path(path_params): Path<OAuthStartPathParams>,
    ValidateJson(request): ValidateJson<StartFileServiceOAuth>,
) -> Result<(StatusCode, Json<OAuthStartResponse>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting cloud file OAuth");

    let provider = path_params.provider;
    let authorization = cloud.oauth_client(provider)?.begin_authorization()?;

    let flow = OAuthFlowState {
        workspace_id: authz.workspace.id,
        workspace_slug: authz.workspace.slug.to_string(),
        account_id: authz.account_id,
        provider,
        display_name: request.display_name,
        root: request.root,
        pkce_verifier: authorization.pkce_verifier,
    };
    let store = nats.kv_store::<OAuthStateBucket>().await?;
    store
        .put(&OAuthStateKey(authorization.csrf_state), &flow)
        .await?;

    Ok((
        StatusCode::OK,
        Json(OAuthStartResponse {
            authorize_url: authorization.authorize_url,
        }),
    ))
}

fn start_oauth_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start cloud file OAuth")
        .description(
            "Begins the OAuth authorization for a cloud file-service connection and returns the \
             provider authorize URL to redirect the user to. On the user's consent, the provider \
             redirects to the callback, which creates the connection.",
        )
        .response::<200, Json<OAuthStartResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Completes a cloud file-service OAuth authorization: the provider redirects
/// here with the code and state.
#[tracing::instrument(skip_all)]
async fn oauth_callback(
    State(pg_client): State<PgClient>,
    State(nats): State<NatsClient>,
    State(crypto): State<CryptoService>,
    State(cloud): State<FileService>,
    State(redirect): State<FileServiceRedirect>,
    security: SecurityContext,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    tracing::debug!(target: TRACING_TARGET, "Completing cloud file OAuth");

    // The callback is a top-level browser navigation, so its outcome is conveyed
    // by a redirect back to the frontend rather than a response body.
    let outcome = complete_callback(&pg_client, &nats, &crypto, &cloud, &security, query).await;
    let (status, workspace_slug) = match outcome {
        Ok(outcome) => {
            tracing::info!(
                target: TRACING_TARGET,
                connection_id = %outcome.connection_id,
                "Cloud file connection created via OAuth",
            );
            ("success", Some(outcome.workspace_slug))
        }
        Err(err) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Cloud file OAuth failed");
            ("error", None)
        }
    };
    connection_result_redirect(redirect.0.as_deref(), status, workspace_slug.as_deref())
}

/// Runs the callback's work: consume the pending authorization, exchange the
/// code, and create the connection. Returns the new connection id and the
/// workspace slug (for the post-auth redirect) on success.
async fn complete_callback(
    pg_client: &PgClient,
    nats: &NatsClient,
    crypto: &CryptoService,
    cloud: &FileService,
    security: &SecurityContext,
    query: OAuthCallbackQuery,
) -> Result<CallbackOutcome> {
    // Validate the state before touching the store so a malformed value maps to
    // a clean BadRequest rather than a KV error.
    let key = OAuthStateKey::from_str(&query.state)
        .map_err(|_| ErrorKind::BadRequest.with_message("Invalid authorization state"))?;

    // Consume the pending authorization single-use: read then delete, so a
    // replayed callback finds nothing and a denial still cleans up its state. A
    // missing entry means an unknown, expired, or already-used state.
    let store = nats.kv_store::<OAuthStateBucket>().await?;
    let flow = store
        .get_value(&key)
        .await?
        .ok_or_else(|| ErrorKind::BadRequest.with_message("Invalid or expired authorization"))?;
    store.delete(&key).await?;

    // A denial (or any provider error) arrives with `error` and no `code`; the
    // state is now consumed, so reject after cleanup.
    if let Some(error) = query.error {
        return Err(ErrorKind::BadRequest
            .with_message("Authorization was denied")
            .with_context(error));
    }
    let code = query
        .code
        .ok_or_else(|| ErrorKind::BadRequest.with_message("Authorization callback missing code"))?;

    let tokens = cloud
        .oauth_client(flow.provider)?
        .exchange_code(code, flow.pkce_verifier)
        .await?;

    let settings = ConnectionSettings {
        tokens,
        root: flow.root,
    };
    let config = ConnectionConfig::FileService(FileServiceConfig::new(flow.provider, settings));
    let provider = config.provider_id().to_owned();
    let connection_type = config.connection_type();
    let encrypted_data = crypto.encrypt_json(flow.workspace_id, &config)?;

    let new_connection = NewWorkspaceConnection {
        workspace_id: flow.workspace_id,
        account_id: flow.account_id,
        display_name: flow.display_name,
        provider,
        connection_type,
        encrypted_data,
        is_active: Some(true),
        metadata: None,
    };

    // Insert the connection and the outbox event atomically. A file service is
    // request-time only — its import is picker-driven and its export per file,
    // neither scheduled — so it gets no schedule row (scheduling is an
    // object-store concept). Transfer capability comes from its provider_type.
    // The requester's security context comes from the callback request itself.
    let mut conn = pg_client.get_connection().await?;
    let connection_id = conn
        .transaction(async |conn| {
            let connection = conn.create_workspace_connection(new_connection).await?;
            conn.emit_event(
                event::EventOrigin {
                    workspace_id: flow.workspace_id,
                    account_id: flow.account_id,
                    security,
                },
                event::WorkspaceEvent::ConnectionCreated(event::ConnectionCreated {
                    connection_id: connection.id,
                    connection_name: connection.display_name.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(connection.id)
        })
        .await?;

    Ok(CallbackOutcome {
        connection_id,
        workspace_slug: flow.workspace_slug,
    })
}

/// Returns the authenticated cloud file OAuth routes: starting an
/// authorization is workspace-scoped and requires `ManageConnections`.
pub fn private_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/connections/oauth/{provider}/start/",
            post_with(start_oauth, start_oauth_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}

/// Returns the public cloud file OAuth routes: the provider's browser redirect
/// lands on the callback with no `Authorization` header, so it must be
/// unauthenticated. Its security rests on the single-use, unguessable CSRF state
/// it consumes.
pub fn public_routes() -> ApiRouter<ServiceState> {
    // A plain axum `route` (not aide's `api_route`), so the callback is absent
    // from the OpenAPI spec entirely. It is a provider-driven browser redirect,
    // never a request an SDK/webapp client issues, so it has no place in the
    // generated client and no reason to appear in the API contract.
    ApiRouter::new().route("/connections/oauth/callback/", get(oauth_callback))
}
