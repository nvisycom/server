//! Connection sync handlers: import from and export to a connection.
//!
//! A sync moves files between a workspace's external connection and the internal
//! file store. Object stores sync by listing: a manual or scheduled trigger
//! imports every new object or exports every redacted output. File services sync
//! by explicit selection: import the files chosen in the provider's picker,
//! export a chosen set of workspace files. Import and export are mirror endpoints
//! — both connection-scoped and taking a batch. Every trigger opens a
//! [`WorkspaceConnectionSync`] and performs the transfer in the background;
//! clients poll the sync detail endpoint for completion.
//!
//! [`WorkspaceConnectionSync`]: nvisy_postgres::model::WorkspaceConnectionSync

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceConnectionSync, WorkspaceConnection};
use nvisy_postgres::query::{
    WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository,
    WorkspaceConnectionSyncRepository,
};
use nvisy_postgres::types::{
    ConnectionId, SyncDeletionPolicy, SyncMode, SyncStatus, SyncTriggerType,
};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{Authorized, Json, Path, Query, ValidateJson, markers};
use crate::handler::request::{
    ConnectionPathParams, ConnectionSyncPathParams, CursorPagination, ExportFiles, ImportFiles,
    WorkspaceSyncsQuery,
};
use crate::handler::response::{ConnectionSync, ConnectionSyncsPage, Page};
use crate::handler::utility::resolve_account_ref;
use crate::response::{Error, ErrorKind, ErrorResponse, Result};
use crate::service::{
    ConnectionConfig, ConnectionSyncService, CryptoService, ServiceState, SourceEntry,
    TransferKind, TransferRequest,
};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connection_syncs";

/// Triggers a sync between the connection and the workspace file store.
///
/// Decrypts the stored connection config, opens a `Manual` sync run, and
/// performs the import or export in the background. Returns `202 Accepted` with
/// the created sync immediately; poll the sync detail endpoint for completion.
/// Requires `RunConnectionSyncs` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn sync_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(connection_sync): State<ConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<(StatusCode, Json<ConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Triggering connection sync");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    if !connection.is_active {
        return Err(ErrorKind::BadRequest.with_message("Connection is not active"));
    }

    // Reject a new sync while one is already running for this connection.
    if let Some(latest) = conn
        .find_latest_workspace_connection_sync(connection.id)
        .await?
        && latest.status.is_in_progress()
    {
        return Err(ErrorKind::Conflict.with_message("A sync is already in progress"));
    }

    let config: ConnectionConfig = crypto.decrypt_json(workspace.id, &connection.encrypted_data)?;
    // This endpoint runs a whole-listing sync, which is the object-store model. A
    // file service transfers on demand (picker import, per-file export) and has no
    // whole-listing sync, so it is rejected here — schedulability is exactly the
    // "can be run as a whole-listing/scheduled sync" capability.
    if !config.supports_schedule() {
        return Err(ErrorKind::BadRequest.with_message(
            "This connection has no whole-listing sync; file services import via the \
             picker and export per file",
        ));
    }

    // The direction and deletion policy come from the connection's schedule when
    // it has one; an object store created without a `sync` block has no schedule
    // row, so a manual run falls back to the defaults (import, ignore deletions).
    let schedule = conn.find_connection_schedule(connection.id).await?;
    let sync_mode = schedule.as_ref().map_or(SyncMode::Import, |s| s.sync_mode);
    let deletion_policy = schedule
        .as_ref()
        .map_or(SyncDeletionPolicy::Ignore, |s| s.deletion_policy);

    // A manual trigger runs the connection's configured direction: import pulls
    // the whole listing; export pushes every redacted output not yet exported.
    let kind = match sync_mode {
        SyncMode::Import => TransferKind::ImportAll { deletion_policy },
        SyncMode::Export => TransferKind::ExportRedacted,
    };
    let sync = open_run_and_transfer(
        &mut conn,
        &connection_sync,
        account_id,
        connection,
        config,
        kind,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(sync)))
}

fn sync_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Sync connection")
        .description(
            "Runs the object-store connection's configured direction: imports every new object, \
             or exports every redacted output not yet exported. File services use the picker \
             import and per-file export instead. Returns the created sync; poll it for completion.",
        )
        .response::<202, Json<ConnectionSync>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Imports a caller-selected set of files from a file-service connection.
///
/// The frontend runs the provider's native picker and posts the chosen files
/// (each an opaque provider id plus display name). Only the selected files are
/// imported; already-imported files are skipped, so re-picking is idempotent.
/// The import runs in the background — returns `202 Accepted` with the created
/// sync; poll the sync detail endpoint for completion. Object-store connections
/// are rejected (they have no picker; use the scheduled or manual import).
/// Requires `RunConnectionSyncs` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn import_files(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(connection_sync): State<ConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<ConnectionPathParams>,
    ValidateJson(request): ValidateJson<ImportFiles>,
) -> Result<(StatusCode, Json<ConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Importing selected files from connection");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    if !connection.is_active {
        return Err(ErrorKind::BadRequest.with_message("Connection is not active"));
    }

    // Reject a new sync while one is already running for this connection.
    if let Some(latest) = conn
        .find_latest_workspace_connection_sync(connection.id)
        .await?
        && latest.status.is_in_progress()
    {
        return Err(ErrorKind::Conflict.with_message("A sync is already in progress"));
    }

    let config: ConnectionConfig = crypto.decrypt_json(workspace.id, &connection.encrypted_data)?;
    if !config.is_file_service() {
        return Err(
            ErrorKind::BadRequest.with_message("Picker import is only available for file services")
        );
    }

    let entries = request
        .files
        .into_iter()
        .map(|file| SourceEntry {
            key: file.id,
            name: file.name,
        })
        .collect();
    let kind = TransferKind::ImportSelected { entries };
    let sync = open_run_and_transfer(
        &mut conn,
        &connection_sync,
        account_id,
        connection,
        config,
        kind,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(sync)))
}

fn import_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Import selected files")
        .description(
            "Imports the files selected in the provider's picker (file services only). \
             Already-imported files are skipped. Returns the created sync; poll it for completion.",
        )
        .response::<202, Json<ConnectionSync>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Exports a caller-selected set of workspace files to a connection, each as a
/// new provider file.
///
/// The mirror of the picker import: connection-scoped, taking a batch of file
/// ids. Each file is written as a new file (a file service never overwrites a
/// source; an object store writes under an `exports/` prefix). The export runs in
/// the background — returns `202 Accepted` with the created sync; poll the sync
/// detail endpoint for completion. Requires `RunConnectionSyncs` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn export_files(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(connection_sync): State<ConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<ConnectionPathParams>,
    ValidateJson(request): ValidateJson<ExportFiles>,
) -> Result<(StatusCode, Json<ConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Exporting selected files to connection");

    let account_id = authz.account_id;
    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    if !connection.is_active {
        return Err(ErrorKind::BadRequest.with_message("Connection is not active"));
    }

    // Reject a new sync while one is already running for this connection.
    if let Some(latest) = conn
        .find_latest_workspace_connection_sync(connection.id)
        .await?
        && latest.status.is_in_progress()
    {
        return Err(ErrorKind::Conflict.with_message("A sync is already in progress"));
    }

    let config: ConnectionConfig = crypto.decrypt_json(workspace.id, &connection.encrypted_data)?;

    // The files are resolved (and missing ids skipped) inside the transfer.
    let kind = TransferKind::ExportSelected {
        file_ids: request.file_ids,
    };
    let sync = open_run_and_transfer(
        &mut conn,
        &connection_sync,
        account_id,
        connection,
        config,
        kind,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(sync)))
}

fn export_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Export files to connection")
        .description(
            "Exports the selected workspace files to the connection, each as a new provider \
             file. Returns the created sync; poll it for completion.",
        )
        .response::<202, Json<ConnectionSync>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Opens a `Manual` sync run for `connection` and drives `kind` in the
/// background, returning the created sync. Shared by every trigger endpoint: the
/// transfer runs in an inner task so a panic is recorded as a failed sync rather
/// than leaving the run stuck in `Running`.
async fn open_run_and_transfer(
    conn: &mut PgConn,
    connection_sync: &ConnectionSyncService,
    account_id: Uuid,
    connection: WorkspaceConnection,
    config: ConnectionConfig,
    kind: TransferKind,
) -> Result<ConnectionSync> {
    let new_run = NewWorkspaceConnectionSync {
        connection_id: connection.id,
        account_id,
        trigger_type: Some(SyncTriggerType::OnDemand),
        status: Some(SyncStatus::Running),
        records_synced: Some(0),
        attempt: Some(1),
        metadata: None,
    };
    let run = connection_sync
        .create_run(conn, new_run, &connection)
        .await?;

    let connection_sync = connection_sync.clone();
    let run_id = run.id;
    tokio::spawn(async move {
        connection_sync
            .run_transfer(TransferRequest {
                run_id,
                connection,
                config,
                account_id,
                kind,
            })
            .await;
    });

    let trigger = resolve_account_ref(conn, run.account_id).await?;
    Ok(ConnectionSync::from_model(run, trigger))
}

/// Lists sync runs for a connection, most recent first.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn list_connection_syncs(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<ConnectionPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<ConnectionSyncsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing connection syncs");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    let page = conn
        .cursor_list_workspace_connection_syncs(connection.id, pagination.into_cursor(), None)
        .await?;

    let page = Page::from_cursor_page(page, |wc| {
        ConnectionSync::from_model(wc.item, wc.account.into())
    });

    Ok((StatusCode::OK, Json(page)))
}

fn list_connection_syncs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connection syncs")
        .description("Returns the connection's sync history, most recent first.")
        .response::<200, Json<ConnectionSyncsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists sync runs across every connection in the workspace, most recent first.
///
/// Optional `status` and repeatable `provider` query filters narrow the result
/// (a sync matches if its connection uses any of the given providers). Requires
/// `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_workspace_syncs(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewConnections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceSyncsQuery>,
) -> Result<(StatusCode, Json<ConnectionSyncsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace syncs");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_workspace_connection_syncs_all(
            workspace.id,
            pagination.into_cursor(),
            query.status,
            &query.provider,
        )
        .await?;

    let page = Page::from_cursor_page(page, |(wc, _connection_id)| {
        ConnectionSync::from_model(wc.item, wc.account.into())
    });

    Ok((StatusCode::OK, Json(page)))
}

fn list_workspace_syncs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace syncs")
        .description(
            "Returns all sync runs across the workspace's connections, most recent first, \
             with optional status and provider filters.",
        )
        .response::<200, Json<ConnectionSyncsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Retrieves a single sync run for a connection.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
        sync_id = %path_params.sync_id,
    )
)]
async fn read_connection_sync(
    State(pg_client): State<PgClient>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<ConnectionSyncPathParams>,
) -> Result<(StatusCode, Json<ConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading connection sync");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    // Confirm the connection is in this workspace before exposing its run.
    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    let run = conn
        .find_connection_sync_in_workspace(workspace.id, path_params.sync_id)
        .await?
        .filter(|run| run.connection_id == connection.id)
        .ok_or_else(|| Error::not_found("connection_sync"))?;

    let trigger = resolve_account_ref(&mut conn, run.account_id).await?;

    Ok((
        StatusCode::OK,
        Json(ConnectionSync::from_model(run, trigger)),
    ))
}

fn read_connection_sync_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection sync")
        .description("Returns a single sync run for the connection.")
        .response::<200, Json<ConnectionSync>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Cancels an in-progress sync run.
///
/// Transitions the run to `Cancelled` if it is still pending or running; a run
/// that already finished is returned unchanged as a `409 Conflict`. The
/// background transfer is bounded by the sync timeout and its completion is
/// status-guarded, so a cancelled run is never overwritten. Requires
/// `RunConnectionSyncs` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        connection_id = %path_params.connection_id,
        sync_id = %path_params.sync_id,
    )
)]
async fn cancel_connection_sync(
    State(pg_client): State<PgClient>,
    State(connection_sync): State<ConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<ConnectionSyncPathParams>,
) -> Result<(StatusCode, Json<ConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Cancelling connection sync");

    let workspace = authz.workspace;
    let mut conn = pg_client.get_connection().await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    // Resolve the run within the workspace + connection before mutating it.
    let run = conn
        .find_connection_sync_in_workspace(workspace.id, path_params.sync_id)
        .await?
        .filter(|run| run.connection_id == connection.id)
        .ok_or_else(|| Error::not_found("connection_sync"))?;

    let cancelled = conn
        .cancel_workspace_connection_sync(run.id)
        .await?
        .ok_or_else(|| {
            ErrorKind::Conflict.with_message("Sync is not in progress and cannot be cancelled")
        })?;

    // Signal the transfer to stop if it is running on this instance. A run on
    // another instance is stopped by the status flip above plus the
    // status-guarded finalizers.
    connection_sync.cancel_local(run.id);

    let trigger = resolve_account_ref(&mut conn, cancelled.account_id).await?;

    Ok((
        StatusCode::OK,
        Json(ConnectionSync::from_model(cancelled, trigger)),
    ))
}

fn cancel_connection_sync_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Cancel connection sync")
        .description("Cancels an in-progress sync run. A run that already finished returns 409.")
        .response::<200, Json<ConnectionSync>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Finds a connection within a workspace by id, or returns a NotFound error.
async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_id: ConnectionId,
) -> Result<WorkspaceConnection> {
    conn.find_connection_in_workspace(workspace_id, connection_id.as_uuid())
        .await?
        .ok_or_else(|| Error::not_found("connection"))
}

/// Returns routes for connection sync operations.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/syncs/",
            get_with(list_workspace_syncs, list_workspace_syncs_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/sync/",
            post_with(sync_connection, sync_connection_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/import/",
            post_with(import_files, import_files_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/export/",
            post_with(export_files, export_files_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/syncs/",
            get_with(list_connection_syncs, list_connection_syncs_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/syncs/{syncId}/",
            get_with(read_connection_sync, read_connection_sync_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/syncs/{syncId}/cancel/",
            post_with(cancel_connection_sync, cancel_connection_sync_docs),
        )
        .with_path_items(|item| item.tag("Connection Syncs"))
}
