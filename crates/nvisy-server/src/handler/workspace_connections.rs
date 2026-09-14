//! Workspace connection management handlers.
//!
//! This module provides workspace connection management functionality,
//! allowing workspace members to create, configure, and manage encrypted
//! provider connections. All operations are secured with proper authorization
//! and follow role-based access control principles.
//!
//! # Encryption
//!
//! A connection's provider config (credentials plus provider-specific settings)
//! is encrypted using workspace-derived keys (HKDF-SHA256 with
//! XChaCha20-Poly1305). The encrypted data is stored in the database and never
//! exposed through the API. Sync state lives in separate tables, not the
//! encrypted blob.
//!
//! The CRUD orchestration lives in [`WorkspaceConnectionService`]; the handler
//! resolves the service, maps its result to a response, and owns the
//! external-store actions (verify, picker token) that reach a provider directly.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::header::CACHE_CONTROL;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use nvisy_file_service::FileService;
use nvisy_postgres::PgClient;

use crate::domain;
use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceConnection, CursorPagination, UpdateWorkspaceConnection,
    WorkspaceConnectionPathParams, WorkspaceConnectionsQuery, WorkspacePickerTokenRequest,
};
use crate::handler::response::{
    WorkspaceConnection, WorkspaceConnectionVerification, WorkspaceConnectionsPage,
    WorkspacePickerToken,
};
use crate::response::{ErrorKind, ErrorResponse, Result};
use crate::service::{
    ConnectionConfig, CryptoService, ExternalObjectStore, ServiceState, event,
    persist_refreshed_tokens,
};

/// Tracing target for workspace connection operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connections";

/// Creates a new workspace connection.
///
/// Returns the connection metadata (without encrypted data). Requires
/// `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn create_connection(
    State(connections): State<domain::WorkspaceConnectionService>,
    authz: Authorized<markers::ManageConnections>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceConnection>,
) -> Result<(StatusCode, Json<WorkspaceConnection>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace connection");

    let workspace = authz.workspace;
    let found = connections
        .create(
            event::EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            request.into(),
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceConnection::from_model(
            found.connection.item,
            workspace.id,
            workspace.handle,
            found.connection.account.into(),
            found.schedule,
            found.last_synced_at,
        )),
    ))
}

fn create_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create connection")
        .description(
            "Creates a new provider connection for the workspace. WorkspaceConnection data is encrypted \
             and stored securely. The response includes connection metadata but never exposes \
             the encrypted credentials.",
        )
        .response::<201, Json<WorkspaceConnection>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all connections for a workspace.
///
/// Returns connection metadata (without encrypted data). Requires
/// `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_connections(
    State(connections): State<domain::WorkspaceConnectionService>,
    authz: Authorized<markers::ViewConnections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceConnectionsQuery>,
) -> Result<(StatusCode, Json<WorkspaceConnectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace connections");

    let workspace = authz.workspace;
    let page = connections
        .list(workspace.id, pagination.into_cursor(), &query.provider)
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceConnectionsPage::from_cursor_page(page, |entry| {
            WorkspaceConnection::from_model(
                entry.connection.item,
                workspace.id,
                workspace.handle.clone(),
                entry.connection.account.into(),
                entry.schedule,
                entry.last_synced_at,
            )
        })),
    ))
}

fn list_connections_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connections")
        .description(
            "Returns all configured connections for the workspace. Only metadata is returned; \
             encrypted credentials are never exposed.",
        )
        .response::<200, Json<WorkspaceConnectionsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a specific workspace connection.
///
/// Returns connection metadata (without encrypted data). Requires
/// `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn read_connection(
    State(connections): State<domain::WorkspaceConnectionService>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceConnection>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace connection");

    let workspace = authz.workspace;
    let found = connections
        .find(workspace.id, path_params.connection_id)
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceConnection::from_model(
            found.connection.item,
            workspace.id,
            workspace.handle,
            found.connection.account.into(),
            found.schedule,
            found.last_synced_at,
        )),
    ))
}

fn read_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection")
        .description("Returns connection metadata without encrypted credentials.")
        .response::<200, Json<WorkspaceConnection>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace connection.
///
/// Updates connection configuration. Requires `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn update_connection(
    State(connections): State<domain::WorkspaceConnectionService>,
    authz: Authorized<markers::ManageConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspaceConnection>,
) -> Result<(StatusCode, Json<WorkspaceConnection>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace connection");

    let workspace = authz.workspace;
    let found = connections
        .update(
            event::EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            path_params.connection_id,
            request.into(),
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceConnection::from_model(
            found.connection.item,
            workspace.id,
            workspace.handle,
            found.connection.account.into(),
            found.schedule,
            found.last_synced_at,
        )),
    ))
}

fn update_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update connection")
        .description("Updates connection name or encrypted data.")
        .response::<200, Json<WorkspaceConnection>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Deletes a workspace connection.
///
/// Soft-deletes the connection. Requires `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn delete_connection(
    State(connections): State<domain::WorkspaceConnectionService>,
    authz: Authorized<markers::ManageConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace connection");

    let workspace = authz.workspace;
    connections
        .delete(
            event::EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            path_params.connection_id,
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn delete_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete connection")
        .description("Soft-deletes the connection from the workspace.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Verifies that a connection's backing object store is reachable.
///
/// Decrypts the stored connection config and attempts a lightweight reachability
/// check against the provider. Returns `200` with a [`WorkspaceConnectionVerification`]
/// describing the outcome: a store that is reachable but rejects the
/// credentials reports `reachable: false` with the reason, rather than an HTTP
/// error. Requires `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn verify_connection(
    State(pg_client): State<PgClient>,
    State(connections): State<domain::WorkspaceConnectionService>,
    State(crypto): State<CryptoService>,
    State(object): State<ExternalObjectStore>,
    State(cloud): State<FileService>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceConnectionVerification>)> {
    tracing::debug!(target: TRACING_TARGET, "Verifying workspace connection");

    let workspace = authz.workspace;

    // Load the connection through the service, then run the provider I/O below
    // with no pooled connection held: `connect`/`verify` reach external services
    // with no total timeout, so holding one could exhaust the pool.
    let connection = connections
        .find(workspace.id, path_params.connection_id)
        .await?
        .connection
        .item;

    let config: ConnectionConfig = crypto.decrypt_json(workspace.id, &connection.encrypted_data)?;

    // Verification is capability-specific: object stores check reachability;
    // inference providers verify their credentials.
    let verification = match config {
        ConnectionConfig::ObjectStore(config) => match object.connect(&config).await {
            Ok(client) => match client.verify_reachable().await {
                Ok(()) => {
                    tracing::info!(target: TRACING_TARGET, "WorkspaceConnection verified");
                    WorkspaceConnectionVerification::reachable()
                }
                Err(err) => {
                    // Log the full error, but return only a safe kind-based reason
                    // so backend URLs/bucket names are not exposed to the client.
                    tracing::warn!(target: TRACING_TARGET, error = %err, "WorkspaceConnection unreachable");
                    WorkspaceConnectionVerification::unreachable(err.kind().reason())
                }
            },
            Err(err) => {
                tracing::warn!(target: TRACING_TARGET, error = %err, "WorkspaceConnection setup failed");
                WorkspaceConnectionVerification::unreachable(err.kind().reason())
            }
        },
        ConnectionConfig::FileService(config) => match cloud.connect(&config).await {
            Ok(connected) => {
                // A refresh during verification produces fresh tokens; persist
                // them so the renewed credentials are not thrown away. Acquire a
                // connection only for this write and release it before the
                // provider `verify` I/O below.
                if let Some(refreshed) = connected.refreshed {
                    let mut conn = pg_client.get_connection().await?;
                    persist_refreshed_tokens(
                        &mut conn,
                        &crypto,
                        workspace.id,
                        connection.id,
                        refreshed.tokens().clone(),
                    )
                    .await?;
                }
                match connected.client.verify().await {
                    Ok(()) => {
                        tracing::info!(target: TRACING_TARGET, "WorkspaceConnection verified");
                        WorkspaceConnectionVerification::reachable()
                    }
                    Err(err) => {
                        tracing::warn!(target: TRACING_TARGET, error = %err, "WorkspaceConnection unreachable");
                        WorkspaceConnectionVerification::unreachable(err.kind().reason())
                    }
                }
            }
            Err(err) => {
                tracing::warn!(target: TRACING_TARGET, error = %err, "WorkspaceConnection setup failed");
                WorkspaceConnectionVerification::unreachable(
                    "credentials rejected or provider unreachable",
                )
            }
        },
    };

    Ok((StatusCode::OK, Json(verification)))
}

fn verify_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Verify connection")
        .description("Checks whether the connection's backing store is reachable with its stored credentials.")
        .response::<200, Json<WorkspaceConnectionVerification>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Mints a short-lived provider OAuth access token for a browser file picker.
///
/// The native pickers (Google Picker, the `OneDrive` and Box file pickers) run in
/// the browser and need a provider access token to do so. Refresh tokens stay
/// server-side, so this returns only a short-lived access token (refreshing it
/// from the stored credentials if the current one has expired, and persisting
/// the refreshed set) with `Cache-Control: no-store`. Rejects a non-file-service
/// connection, and a Dropbox connection (its Chooser uses a public app key, not a
/// user token). Requires `RunConnectionSyncs` permission, matching the import it
/// precedes.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn mint_picker_token(
    State(pg_client): State<PgClient>,
    State(connections): State<domain::WorkspaceConnectionService>,
    State(crypto): State<CryptoService>,
    State(cloud): State<FileService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    // Optional body: a picker that names a resource per `authenticate` command
    // (OneDrive) sends `{ resource }`; single-token pickers send no body.
    request: Option<ValidateJson<WorkspacePickerTokenRequest>>,
) -> Result<(StatusCode, HeaderMap, Json<WorkspacePickerToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Minting picker token");
    let resource = request.and_then(|ValidateJson(body)| body.resource);

    let workspace = authz.workspace;

    // Load the connection through the service, then run the provider token refresh
    // below with no pooled connection held (it reaches the provider with no total
    // timeout).
    let connection = connections
        .find(workspace.id, path_params.connection_id)
        .await?
        .connection
        .item;

    if !connection.is_active {
        return Err(ErrorKind::BadRequest.with_message("WorkspaceConnection is not active"));
    }

    let config: ConnectionConfig = crypto.decrypt_json(workspace.id, &connection.encrypted_data)?;
    let ConnectionConfig::FileService(config) = config else {
        return Err(ErrorKind::BadRequest
            .with_message("Picker tokens are only available for file services"));
    };
    if !config.provider.picker_needs_user_token() {
        return Err(ErrorKind::BadRequest.with_message(
            "This provider's picker uses a client-side app key, not a server token",
        ));
    }

    // Mint the picker access token for the requested resource. The provider layer
    // decides what the picker needs (OneDrive: a SharePoint-audience token; Drive
    // and Box: the ordinary provider token). The refresh token never leaves the
    // server; a rotated refresh token comes back for persistence.
    let picker = cloud
        .mint_picker_token(&config, resource.as_deref())
        .await?;
    if let Some(refreshed) = picker.refreshed {
        let mut conn = pg_client.get_connection().await?;
        persist_refreshed_tokens(
            &mut conn,
            &crypto,
            workspace.id,
            connection.id,
            refreshed.tokens().clone(),
        )
        .await?;
    }

    // The response carries a bearer credential; keep it out of any cache.
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));

    let token = WorkspacePickerToken {
        access_token: picker.access_token,
        expires_at: picker.expires_at,
    };
    Ok((StatusCode::OK, headers, Json(token)))
}

fn mint_picker_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Mint picker token")
        .description(
            "Returns a short-lived provider access token for a browser file picker (file \
             services with a token-based picker only). The refresh token is never returned. \
             An optional body `{ resource }` names the resource the picker requested (used by \
             the OneDrive picker, which requires a SharePoint-audience token); providers whose \
             picker takes a single token ignore it. The OneDrive picker is available only for \
             work or school (OneDrive for Business) accounts.",
        )
        .response::<200, Json<WorkspacePickerToken>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns routes for workspace connection management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/connections",
            post_with(create_connection, create_connection_docs)
                .get_with(list_connections, list_connections_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}",
            get_with(read_connection, read_connection_docs)
                .patch_with(update_connection, update_connection_docs)
                .delete_with(delete_connection, delete_connection_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/verify",
            post_with(verify_connection, verify_connection_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/picker-token",
            post_with(mint_picker_token, mint_picker_token_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
