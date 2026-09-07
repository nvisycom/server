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
use axum::response::Redirect;
use axum::routing::get;
use nvisy_file_service::FileService;
use nvisy_file_service::provider::{ConnectionSettings, FileServiceConfig, Provider};
use nvisy_nats::NatsClient;
use nvisy_nats::kv::{OAuthStateBucket as OAuthStateKvBucket, OAuthStateKey};
use nvisy_postgres::model::{NewWorkspaceConnection, NewWorkspaceConnectionSchedule};
use nvisy_postgres::query::{WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository};
use nvisy_postgres::types::{SyncDeletionPolicy, SyncMode};
use nvisy_postgres::{AsyncConnection, PgClient};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, SecurityContext, ValidateJson,
    WorkspaceContext,
};
use crate::handler::request::{OAuthCallbackQuery, OAuthStartPathParams, StartFileServiceOAuth};
use crate::handler::response::ErrorResponse;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{
    ConnectionConfig, ConnectionRef, CryptoService, EventEmitter, EventOrigin, FileServiceRedirect,
    ServiceState, WorkspaceEvent,
};

/// Tracing target for connection OAuth operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connection_oauth";

/// The stashed state of an in-flight OAuth authorization, held between `start`
/// and `callback` in the [`OAuthStateBucket`]. Keyed by the CSRF state token.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OAuthFlowState {
    /// Workspace the connection will be created in.
    workspace_id: Uuid,
    /// Account that started the flow; the connection is attributed to it.
    account_id: Uuid,
    /// The provider being connected.
    provider: Provider,
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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        provider = ?path_params.provider,
    )
)]
async fn start_oauth(
    State(pg_client): State<PgClient>,
    State(nats): State<NatsClient>,
    State(cloud): State<FileService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<OAuthStartPathParams>,
    ValidateJson(request): ValidateJson<StartFileServiceOAuth>,
) -> Result<(StatusCode, Json<OAuthStartResponse>)> {
    tracing::debug!(target: TRACING_TARGET, "Starting cloud file OAuth");

    let mut conn = pg_client.get_connection().await?;
    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;
    drop(conn);

    let provider = path_params.provider;
    let authorization = cloud.oauth_client(provider)?.begin_authorization()?;

    let flow = OAuthFlowState {
        workspace_id: workspace.id,
        account_id: auth_state.account_id,
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
) -> Redirect {
    tracing::debug!(target: TRACING_TARGET, "Completing cloud file OAuth");

    // The callback is a top-level browser navigation, so its outcome is conveyed
    // by a redirect back to the frontend rather than a response body.
    let outcome = complete_callback(&pg_client, &nats, &crypto, &cloud, &security, query).await;
    let status = match outcome {
        Ok(connection_id) => {
            tracing::info!(
                target: TRACING_TARGET,
                connection_id = %connection_id,
                "Cloud file connection created via OAuth",
            );
            "success"
        }
        Err(err) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, "Cloud file OAuth failed");
            "error"
        }
    };
    redirect_to_frontend(redirect.0.as_deref(), status)
}

/// Runs the callback's work: consume the pending authorization, exchange the
/// code, and create the connection. Returns the new connection id on success.
async fn complete_callback(
    pg_client: &PgClient,
    nats: &NatsClient,
    crypto: &CryptoService,
    cloud: &FileService,
    security: &SecurityContext,
    query: OAuthCallbackQuery,
) -> Result<Uuid> {
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
    let provider_type = config.provider_type();
    let encrypted_data = crypto.encrypt_json(flow.workspace_id, &config)?;

    let new_connection = NewWorkspaceConnection {
        workspace_id: flow.workspace_id,
        account_id: flow.account_id,
        display_name: flow.display_name,
        provider,
        provider_type,
        encrypted_data,
        is_active: Some(true),
        metadata: None,
    };

    // Insert the connection, its schedule row, and the outbox event atomically,
    // mirroring the ordinary create path. A cloud file service is on-demand (no
    // cron), so the schedule carries no cron; the row marks transfer capability
    // and holds default settings. Its direction is driven per endpoint — the picker
    // for import, an explicit request for export. The requester's security
    // context comes from the callback request itself.
    let mut conn = pg_client.get_connection().await?;
    let connection_id = conn
        .transaction(async |conn| {
            let connection = conn.create_workspace_connection(new_connection).await?;
            conn.create_connection_schedule(NewWorkspaceConnectionSchedule {
                connection_id: connection.id,
                sync_mode: Some(SyncMode::Import),
                schedule_cron: None,
                deletion_policy: Some(SyncDeletionPolicy::default()),
            })
            .await?;
            conn.emit_event(
                EventOrigin {
                    workspace_id: flow.workspace_id,
                    account_id: flow.account_id,
                    security,
                },
                WorkspaceEvent::ConnectionCreated(ConnectionRef {
                    connection_id: connection.id,
                    connection_name: connection.display_name.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>(connection.id)
        })
        .await?;

    Ok(connection_id)
}

/// Redirects the browser back to the frontend with the flow's outcome. Falls
/// back to a self-describing data page when no frontend URL is configured.
fn redirect_to_frontend(base: Option<&str>, status: &str) -> Redirect {
    match base {
        Some(base) => {
            let separator = if base.contains('?') { '&' } else { '?' };
            Redirect::to(&format!("{base}{separator}connection={status}"))
        }
        None => Redirect::to(&format!(
            "data:text/plain,cloud%20file%20connection%20{status}"
        )),
    }
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
