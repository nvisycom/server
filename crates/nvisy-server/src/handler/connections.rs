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

use std::collections::HashMap;

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_inference::Error as InferenceError;
use nvisy_postgres::model::{
    NewWorkspaceConnection, NewWorkspaceConnectionSchedule, UpdateWorkspaceConnection,
    WorkspaceConnection, WorkspaceConnectionSchedule,
};
use nvisy_postgres::query::{
    WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository,
    WorkspaceConnectionSyncRepository,
};
use nvisy_postgres::types::{ConnectionId, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, PgError};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, WorkspaceContext,
};
use crate::handler::request::{
    ConnectionPathParams, ConnectionsQuery, CreateConnection, CursorPagination, SyncScheduleInput,
    UpdateConnection,
};
use crate::handler::response::{
    Connection, ConnectionVerification, ConnectionsPage, ErrorResponse,
};
use crate::handler::utility::resolve_account_ref;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{ConnectionConfig, CryptoService, ObjectService, ServiceState, is_valid_cron};

/// Tracing target for workspace connection operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connections";

/// Creates a new workspace connection.
///
/// Returns the connection metadata (without encrypted data). Requires
/// `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn create_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    ValidateJson(request): ValidateJson<CreateConnection>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    // Sync config applies only to sync-capable providers. Validate the pairing
    // before any write so a mismatch fails fast.
    let supports_sync = request.config.supports_sync();
    if let Some(sync) = &request.sync {
        if !supports_sync {
            return Err(ErrorKind::BadRequest
                .with_message("This provider does not support sync configuration"));
        }
        validate_sync_input(sync)?;
    }

    // The provider column is derived from the typed config so the two can never
    // disagree; the full config is encrypted at rest.
    let provider = request.config.provider_id().to_owned();
    let encrypted_data = crypto.encrypt_json(workspace.id, &request.config)?;

    let new_connection = NewWorkspaceConnection {
        workspace_id: workspace.id,
        account_id: auth_state.account_id,
        display_name: request.display_name,
        provider,
        encrypted_data,
        is_active: request.is_active,
        metadata: None,
    };

    // Insert the connection and (if sync-capable) its schedule atomically, so a
    // partial write can never leave a sync-capable connection without a schedule.
    let sync = request.sync.unwrap_or_default();
    let (connection, schedule) = conn
        .transaction(async |conn| {
            let connection = conn.create_workspace_connection(new_connection).await?;
            // A sync-capable connection gets a schedule row (its presence marks
            // the capability).
            let schedule = if supports_sync {
                Some(
                    conn.create_connection_schedule(NewWorkspaceConnectionSchedule {
                        connection_id: connection.id,
                        sync_mode: Some(sync.sync_mode),
                        schedule_cron: sync.schedule_cron,
                        deletion_policy: Some(sync.deletion_policy),
                    })
                    .await?,
                )
            } else {
                None
            };
            Ok::<_, PgError>((connection, schedule))
        })
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        connection_id = %ConnectionId::from_uuid(connection.id),
        provider = %connection.provider,
        "Connection created",
    );

    // The creator is the authenticated caller, and a fresh connection has no
    // sync runs yet, so last-synced is `None`.
    let creator = resolve_account_ref(&mut conn, auth_state.account_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(Connection::from_model(
            connection,
            workspace.slug,
            creator,
            schedule,
            None,
        )),
    ))
}

fn create_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create connection")
        .description(
            "Creates a new provider connection for the workspace. Connection data is encrypted \
             and stored securely. The response includes connection metadata but never exposes \
             the encrypted credentials.",
        )
        .response::<201, Json<Connection>>()
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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_connections(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<ConnectionsQuery>,
) -> Result<(StatusCode, Json<ConnectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace connections");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let page = conn
        .cursor_list_workspace_connections(workspace.id, pagination.into(), &query.provider)
        .await?;

    // One grouped query resolves last-synced for the whole page (not per row).
    let ids: Vec<Uuid> = page.items.iter().map(|wc| wc.item.id).collect();
    let last_synced: HashMap<Uuid, jiff::Timestamp> = conn
        .last_successful_sync_at(&ids)
        .await?
        .into_iter()
        .map(|(id, ts)| (id, ts.into()))
        .collect();

    // One query resolves the sync schedules for the whole page (present only for
    // sync-capable connections), so the list carries the same sync info as the
    // detail view without a per-row round-trip.
    let mut schedules: HashMap<Uuid, _> = conn
        .find_schedules(&ids)
        .await?
        .into_iter()
        .map(|schedule| (schedule.connection_id, schedule))
        .collect();

    tracing::debug!(
        target: TRACING_TARGET,
        connection_count = page.items.len(),
        "Workspace connections listed",
    );

    Ok((
        StatusCode::OK,
        Json(ConnectionsPage::from_cursor_page(page, |wc| {
            let synced = last_synced.get(&wc.item.id).copied();
            let schedule = schedules.remove(&wc.item.id);
            Connection::from_model(
                wc.item,
                workspace.slug.clone(),
                wc.account.into(),
                schedule,
                synced,
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
        .response::<200, Json<ConnectionsPage>>()
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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn read_connection(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let (found, schedule, last_synced) =
        find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace connection read");

    Ok((
        StatusCode::OK,
        Json(Connection::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
            schedule,
            last_synced,
        )),
    ))
}

fn read_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection")
        .description("Returns connection metadata without encrypted credentials.")
        .response::<200, Json<Connection>>()
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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn update_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
    ValidateJson(request): ValidateJson<UpdateConnection>,
) -> Result<(StatusCode, Json<Connection>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    let existing = find_connection(&mut conn, workspace.id, path_params.connection_id)
        .await?
        .0
        .item;

    // Sync config only applies to sync-capable connections. A connection's
    // capability is fixed by its provider, which the config replacement (if any)
    // must preserve.
    let supports_sync = match &request.config {
        Some(config) => config.supports_sync(),
        None => conn.find_connection_schedule(existing.id).await?.is_some(),
    };
    if let Some(sync) = &request.sync {
        if !supports_sync {
            return Err(ErrorKind::BadRequest
                .with_message("This provider does not support sync configuration"));
        }
        validate_sync_input(sync)?;
    }

    // Replacing the config re-derives the provider column and re-encrypts the
    // blob together, so they stay in lockstep.
    let (provider, encrypted_data) = match &request.config {
        Some(config) => (
            Some(config.provider_id().to_owned()),
            Some(crypto.encrypt_json(workspace.id, config)?),
        ),
        None => (None, None),
    };

    let update_data = UpdateWorkspaceConnection {
        display_name: request.display_name,
        provider,
        is_active: request.is_active,
        encrypted_data,
        ..Default::default()
    };

    // Update the connection and its schedule atomically so a partial write can
    // never leave a sync-capable connection without its schedule.
    let connection_id = existing.id;
    let sync = request.sync;
    conn.transaction(async |conn| {
        conn.update_workspace_connection(connection_id, update_data)
            .await?;
        if let Some(sync) = sync {
            conn.upsert_connection_schedule(NewWorkspaceConnectionSchedule {
                connection_id,
                sync_mode: Some(sync.sync_mode),
                schedule_cron: sync.schedule_cron,
                deletion_policy: Some(sync.deletion_policy),
            })
            .await?;
        }
        Ok::<(), PgError>(())
    })
    .await?;

    let (found, schedule, last_synced) =
        find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    tracing::info!(target: TRACING_TARGET, "Connection updated");

    Ok((
        StatusCode::OK,
        Json(Connection::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
            schedule,
            last_synced,
        )),
    ))
}

fn update_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update connection")
        .description("Updates connection name or encrypted data.")
        .response::<200, Json<Connection>>()
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
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn delete_connection(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    let existing = find_connection(&mut conn, workspace.id, path_params.connection_id)
        .await?
        .0
        .item;

    conn.delete_workspace_connection(existing.id).await?;

    tracing::info!(target: TRACING_TARGET, "Connection deleted");

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
/// check against the provider. Returns `200` with a [`ConnectionVerification`]
/// describing the outcome: a store that is reachable but rejects the
/// credentials reports `reachable: false` with the reason, rather than an HTTP
/// error. Requires `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn verify_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(object): State<ObjectService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<(StatusCode, Json<ConnectionVerification>)> {
    tracing::debug!(target: TRACING_TARGET, "Verifying workspace connection");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id)
        .await?
        .0
        .item;

    let config: ConnectionConfig = crypto.decrypt_json(workspace.id, &connection.encrypted_data)?;

    // Verification is capability-specific: object stores check reachability;
    // inference providers verify their credentials.
    let verification = match config {
        ConnectionConfig::ObjectStore(config) => match object.connect(&config).await {
            Ok(client) => match client.verify_reachable().await {
                Ok(()) => {
                    tracing::info!(target: TRACING_TARGET, "Connection verified");
                    ConnectionVerification::reachable()
                }
                Err(err) => {
                    // Log the full error, but return only a safe kind-based reason
                    // so backend URLs/bucket names are not exposed to the client.
                    tracing::warn!(target: TRACING_TARGET, error = %err, "Connection unreachable");
                    ConnectionVerification::unreachable(err.kind().reason())
                }
            },
            Err(err) => {
                tracing::warn!(target: TRACING_TARGET, error = %err, "Connection setup failed");
                ConnectionVerification::unreachable(err.kind().reason())
            }
        },
        ConnectionConfig::Inference(config) => match config.validate().await {
            Ok(()) => {
                tracing::info!(target: TRACING_TARGET, "Connection verified");
                ConnectionVerification::reachable()
            }
            Err(err) => {
                // Log the full error, but return only a safe, kind-based reason
                // so provider endpoints/keys are not echoed to the client.
                tracing::warn!(target: TRACING_TARGET, error = %err, "Connection verification failed");
                let reason = match err {
                    InferenceError::Build(_) => "invalid configuration",
                    _ => "credentials rejected or provider unreachable",
                };
                ConnectionVerification::unreachable(reason)
            }
        },
    };

    Ok((StatusCode::OK, Json(verification)))
}

fn verify_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Verify connection")
        .description("Checks whether the connection's backing store is reachable with its stored credentials.")
        .response::<200, Json<ConnectionVerification>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Validates a sync-schedule input: a valid cron and, since scheduling is
/// import-only, no cron on an export connection.
fn validate_sync_input(sync: &SyncScheduleInput) -> Result<()> {
    if let Some(cron) = &sync.schedule_cron {
        if !is_valid_cron(cron) {
            return Err(ErrorKind::BadRequest.with_message("Invalid cron expression"));
        }
        if sync.sync_mode.is_export() {
            return Err(
                ErrorKind::BadRequest.with_message("Only import connections can be scheduled")
            );
        }
    }
    Ok(())
}

/// Finds a connection within a workspace by id, with its creator, or returns a
/// NotFound error.
type FoundConnection = (
    WithAccountRef<WorkspaceConnection>,
    Option<WorkspaceConnectionSchedule>,
    Option<jiff::Timestamp>,
);

async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_id: ConnectionId,
) -> Result<FoundConnection> {
    let found = conn
        .find_connection_in_workspace_with_creator(workspace_id, connection_id.as_uuid())
        .await?
        .ok_or_else(|| Error::not_found("connection"))?;
    let schedule = conn.find_connection_schedule(found.item.id).await?;
    let last_synced = conn
        .last_successful_sync_at(&[found.item.id])
        .await?
        .into_iter()
        .next()
        .map(|(_, ts)| ts.into());
    Ok((found, schedule, last_synced))
}

/// Returns routes for workspace connection management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/connections/",
            post_with(create_connection, create_connection_docs)
                .get_with(list_connections, list_connections_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/",
            get_with(read_connection, read_connection_docs)
                .patch_with(update_connection, update_connection_docs)
                .delete_with(delete_connection, delete_connection_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/verify/",
            post_with(verify_connection, verify_connection_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
