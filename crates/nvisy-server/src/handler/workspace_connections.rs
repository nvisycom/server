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
use axum::http::header::CACHE_CONTROL;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use nvisy_core::net::EndpointPolicy;
use nvisy_file_service::FileService;
use nvisy_postgres::model::{
    NewWorkspaceConnection, NewWorkspaceConnectionSchedule,
    WorkspaceConnection as WorkspaceConnectionModel, WorkspaceConnectionSchedule,
};
use nvisy_postgres::query::{
    WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository,
    WorkspaceConnectionSyncRepository,
};
use nvisy_postgres::types::{ConnectionId, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, model};
use uuid::Uuid;

use crate::extract::{Authorized, Json, Path, Query, SecurityContext, ValidateJson, markers};
use crate::handler::request::{
    CreateWorkspaceConnection, CursorPagination, SyncScheduleInput, UpdateWorkspaceConnection,
    WorkspaceConnectionPathParams, WorkspaceConnectionsQuery, WorkspacePickerTokenRequest,
};
use crate::handler::response::{
    WorkspaceConnection, WorkspaceConnectionVerification, WorkspaceConnectionsPage,
    WorkspacePickerToken,
};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{
    ConnectionConfig, ConnectionCreated, ConnectionDeleted, ConnectionUpdated, CryptoService,
    EventEmitter, EventOrigin, ExternalObjectStore, ServiceState, StandardCronSchedule,
    WorkspaceEvent, persist_refreshed_tokens,
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
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(endpoint_policy): State<EndpointPolicy>,
    authz: Authorized<markers::ManageConnections>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceConnection>,
) -> Result<(StatusCode, Json<WorkspaceConnection>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace connection");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Reject a disallowed custom endpoint under the deployment policy before the
    // config is ever stored (SSRF / cleartext-credential guard).
    request.config.validate_endpoints(endpoint_policy).await?;

    // A `sync` block configures a scheduled sync, which only schedulable providers
    // accept (a file service transfers on demand, not on a timer). Validate the
    // pairing before any write so a mismatch fails fast.
    if let Some(sync) = &request.sync {
        if !request.config.supports_schedule() {
            return Err(
                ErrorKind::BadRequest.with_message("This provider does not support scheduled sync")
            );
        }
        validate_sync_input(sync)?;
    }

    // The provider and its capability type are derived from the typed config so
    // they can never disagree with it; the full config is encrypted at rest.
    let provider = request.config.provider_id().to_owned();
    let connection_type = request.config.connection_type();
    let encrypted_data = crypto.encrypt_json(workspace.id, &request.config)?;

    let new_connection = NewWorkspaceConnection {
        workspace_id: workspace.id,
        account_id,
        display_name: request.display_name,
        provider,
        connection_type,
        encrypted_data,
        is_active: request.is_active,
        metadata: None,
    };

    // Insert the connection, its schedule (only when a `sync` block was given for
    // a schedulable provider), and the outbox event atomically, so a partial write
    // can never leave the schedule out of step with the connection, nor record —
    // or lose — the event out of step with the insert. A connection without a
    // schedule row still transfers on demand; the row is purely the cron config.
    let sync = request.sync;
    let (connection, schedule) = conn
        .transaction(async |conn| {
            let connection = conn.create_workspace_connection(new_connection).await?;
            let schedule = match sync {
                Some(sync) => Some(
                    conn.create_connection_schedule(NewWorkspaceConnectionSchedule {
                        connection_id: connection.id,
                        sync_mode: Some(sync.sync_mode),
                        schedule_cron: sync.schedule_cron,
                        deletion_policy: Some(sync.deletion_policy),
                    })
                    .await?,
                ),
                None => None,
            };
            conn.emit_event(
                EventOrigin {
                    workspace_id: workspace.id,
                    account_id,
                    security: &security,
                },
                WorkspaceEvent::ConnectionCreated(ConnectionCreated {
                    connection_id: connection.id,
                    connection_name: connection.display_name.clone(),
                }),
            )
            .await?;
            Ok::<_, Error>((connection, schedule))
        })
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        connection_id = %ConnectionId::from_uuid(connection.id),
        provider = %connection.provider,
        "WorkspaceConnection created",
    );

    // The creator is the authenticated caller, and a fresh connection has no
    // sync runs yet, so last-synced is `None`.
    let creator = resolve_account_ref(&mut conn, account_id).await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceConnection::from_model(
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
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewConnections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceConnectionsQuery>,
) -> Result<(StatusCode, Json<WorkspaceConnectionsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace connections");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_workspace_connections(workspace.id, pagination.into_cursor(), &query.provider)
        .await?;

    // One grouped query resolves last-synced for the whole page (not per row).
    let ids: Vec<Uuid> = page.items.iter().map(|wc| wc.item.id).collect();
    let last_synced_at: HashMap<Uuid, jiff::Timestamp> = conn
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
        Json(WorkspaceConnectionsPage::from_cursor_page(page, |wc| {
            let synced = last_synced_at.get(&wc.item.id).copied();
            let schedule = schedules.remove(&wc.item.id);
            WorkspaceConnection::from_model(
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
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceConnection>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading workspace connection");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let FoundConnection {
        connection: found,
        schedule,
        last_synced_at,
    } = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Workspace connection read");

    Ok((
        StatusCode::OK,
        Json(WorkspaceConnection::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
            schedule,
            last_synced_at,
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
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(endpoint_policy): State<EndpointPolicy>,
    authz: Authorized<markers::ManageConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspaceConnection>,
) -> Result<(StatusCode, Json<WorkspaceConnection>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace connection");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Reject a disallowed custom endpoint on the replacement config before store.
    if let Some(config) = &request.config {
        config.validate_endpoints(endpoint_policy).await?;
    }

    let existing = find_connection(&mut conn, workspace.id, path_params.connection_id)
        .await?
        .connection
        .item;

    // A `sync` block configures a scheduled sync, which only schedulable
    // connection types accept. Schedulability is fixed by the connection's
    // provider (a config replacement must preserve the provider, checked under the
    // lock below), so it is read from the stored `connection_type` without
    // decrypting the config.
    if let Some(sync) = &request.sync {
        if !existing.connection_type.supports_schedule() {
            return Err(
                ErrorKind::BadRequest.with_message("This provider does not support scheduled sync")
            );
        }
        validate_sync_input(sync)?;
    }

    // Update the connection, its schedule, and the outbox event atomically so a
    // partial write can never leave a transfer-capable connection without its
    // schedule, nor record — or lose — the event out of step with the update.
    let connection_id = existing.id;
    // The effective post-update name: the new one if the request set it, else the
    // existing name.
    let connection_name = request
        .display_name
        .clone()
        .unwrap_or_else(|| existing.display_name.clone());
    let crypto = crypto.clone();
    conn.transaction(async move |conn| {
        // Lock the row first so this update serializes against a concurrent
        // token refresh (persist_refreshed_tokens), preventing a lost update to
        // `encrypted_data`. A row deleted since the pre-transaction read is
        // treated as gone.
        let Some(current) = conn
            .find_workspace_connection_by_id_for_update(connection_id)
            .await?
        else {
            return Err(ErrorKind::NotFound.with_message("WorkspaceConnection not found"));
        };

        // Re-encrypt the replacement config under the lock. A connection's
        // provider is fixed at creation, so a config replacement must keep the
        // same provider — changing it would desync the provider/provider_type
        // columns, the schedule, and (for OAuth) the stored tokens. Reject a
        // differing provider rather than silently migrate. For a file-service
        // connection, carry the row's *current* OAuth tokens onto the new config:
        // tokens are never sent by the client (they are not returned by the API),
        // and a refresh may have updated them since this request was built, so a
        // blind full-replace would lose them.
        let (provider, encrypted_data) = match request.config {
            Some(mut config) => {
                let stored: ConnectionConfig =
                    crypto.decrypt_json(workspace.id, &current.encrypted_data)?;
                if config.provider_id() != stored.provider_id() {
                    return Err(ErrorKind::BadRequest.with_message(
                        "A connection's provider cannot be changed; delete and recreate instead",
                    ));
                }
                if let (
                    ConnectionConfig::FileService(new),
                    ConnectionConfig::FileService(existing),
                ) = (&mut config, &stored)
                {
                    new.set_tokens(existing.tokens().clone());
                }
                (
                    Some(config.provider_id().to_owned()),
                    Some(crypto.encrypt_json(workspace.id, &config)?),
                )
            }
            None => (None, None),
        };

        let update_data = model::UpdateWorkspaceConnection {
            display_name: request.display_name,
            provider,
            is_active: request.is_active,
            encrypted_data,
            ..Default::default()
        };
        conn.update_workspace_connection(connection_id, update_data)
            .await?;
        if let Some(sync) = request.sync {
            conn.upsert_connection_schedule(NewWorkspaceConnectionSchedule {
                connection_id,
                sync_mode: Some(sync.sync_mode),
                schedule_cron: sync.schedule_cron,
                deletion_policy: Some(sync.deletion_policy),
            })
            .await?;
        }
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            WorkspaceEvent::ConnectionUpdated(ConnectionUpdated {
                connection_id,
                connection_name,
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    let FoundConnection {
        connection: found,
        schedule,
        last_synced_at,
    } = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    tracing::info!(target: TRACING_TARGET, "WorkspaceConnection updated");

    Ok((
        StatusCode::OK,
        Json(WorkspaceConnection::from_model(
            found.item,
            workspace.slug,
            found.account.into(),
            schedule,
            last_synced_at,
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
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ManageConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting workspace connection");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let existing = find_connection(&mut conn, workspace.id, path_params.connection_id)
        .await?
        .connection
        .item;

    // Delete the connection and record the outbox event atomically, so the event
    // is never lost, nor recorded for a delete that rolled back.
    conn.transaction(async |conn| {
        conn.delete_workspace_connection(existing.id).await?;
        conn.emit_event(
            EventOrigin {
                workspace_id: workspace.id,
                account_id,
                security: &security,
            },
            WorkspaceEvent::ConnectionDeleted(ConnectionDeleted {
                connection_id: existing.id,
                connection_name: existing.display_name.clone(),
            }),
        )
        .await?;
        Ok::<(), Error>(())
    })
    .await?;

    tracing::info!(target: TRACING_TARGET, "WorkspaceConnection deleted");

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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn verify_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(object): State<ExternalObjectStore>,
    State(cloud): State<FileService>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceConnectionVerification>)> {
    tracing::debug!(target: TRACING_TARGET, "Verifying workspace connection");

    let workspace = authz.workspace;

    // Do the DB work up front, then release the connection before the provider
    // I/O below. `connect`/`verify` reach external services with no total
    // timeout, so holding a pooled connection across them could exhaust the pool.
    let connection = {
        let mut conn = pg_client.get_connection().await?;
        find_connection(&mut conn, workspace.id, path_params.connection_id)
            .await?
            .connection
            .item
    };

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
/// The native pickers (Google Picker, the OneDrive and Box file pickers) run in
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

    // Do the DB work up front, then release the connection before the provider
    // token refresh below (which reaches the provider with no total timeout).
    let connection = {
        let mut conn = pg_client.get_connection().await?;
        find_connection(&mut conn, workspace.id, path_params.connection_id)
            .await?
            .connection
            .item
    };

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

/// Validates a sync-schedule input: the cron expression, when present, must be
/// valid. Either direction may be scheduled — an import pulls the listing, an
/// export pushes redacted outputs.
fn validate_sync_input(sync: &SyncScheduleInput) -> Result<()> {
    if let Some(cron) = &sync.schedule_cron
        && !StandardCronSchedule.is_valid(cron)
    {
        return Err(ErrorKind::BadRequest.with_message("Invalid cron expression"));
    }
    Ok(())
}

/// A connection found within a workspace, with its creator, schedule, and last
/// successful sync time.
struct FoundConnection {
    /// The connection paired with its creator account reference.
    connection: WithAccountRef<WorkspaceConnectionModel>,
    /// The sync schedule, present only for transfer-capable connections.
    schedule: Option<WorkspaceConnectionSchedule>,
    /// When the connection last synced successfully, if ever.
    last_synced_at: Option<jiff::Timestamp>,
}

/// Finds a connection within a workspace by id, with its creator, or returns a
/// NotFound error.
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
    let last_synced_at = conn
        .last_successful_sync_at(&[found.item.id])
        .await?
        .into_iter()
        .next()
        .map(|(_, ts)| ts.into());
    Ok(FoundConnection {
        connection: found,
        schedule,
        last_synced_at,
    })
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
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/picker-token/",
            post_with(mint_picker_token, mint_picker_token_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
