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
use nvisy_postgres::model::WorkspaceConnectionSync as WorkspaceConnectionSyncModel;
use nvisy_postgres::types::WithAccountRef;

use crate::domain::{self, SyncRequest};
use crate::extract::{Authorized, Json, Path, Query, ValidateJson, markers};
use crate::handler::request::{
    CursorPagination, ExportWorkspaceFiles, ImportWorkspaceFiles, WorkspaceConnectionPathParams,
    WorkspaceConnectionSyncPathParams, WorkspaceSyncsQuery,
};
use crate::handler::response::{Page, WorkspaceConnectionSync, WorkspaceConnectionSyncsPage};
use crate::response::{ErrorResponse, Result};
use crate::service::ServiceState;
use crate::worker::integration::SourceEntry;

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::handler::workspace_connection_syncs";

/// Triggers a sync between the connection and the workspace file store.
///
/// Decrypts the stored connection config, opens an on-demand sync run, and
/// performs the import or export in the background. Returns `202 Accepted` with
/// the created sync immediately; poll the sync detail endpoint for completion.
/// Requires `RunConnectionSyncs` permission.
#[tracing::instrument(
    skip_all,
    fields(
        connection_id = %path_params.connection_id,
    )
)]
async fn sync_connection(
    State(syncs): State<domain::WorkspaceConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
) -> Result<(StatusCode, Json<WorkspaceConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Triggering connection sync");
    let request = SyncRequest::Configured;
    let run = syncs
        .start(authz.origin(), path_params.connection_id.as_uuid(), request)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(WorkspaceConnectionSync::from_model(
            run,
            authz.account.clone().into(),
        )),
    ))
}

fn sync_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Sync connection")
        .description(
            "Runs the object-store connection's configured direction: imports every new object, \
             or exports every redacted output not yet exported. File services use the picker \
             import and per-file export instead. Returns the created sync; poll it for completion.",
        )
        .response::<202, Json<WorkspaceConnectionSync>>()
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
        connection_id = %path_params.connection_id,
    )
)]
async fn import_files(
    State(syncs): State<domain::WorkspaceConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    ValidateJson(request): ValidateJson<ImportWorkspaceFiles>,
) -> Result<(StatusCode, Json<WorkspaceConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Importing selected files from connection");
    let entries = request
        .files
        .into_iter()
        .map(|file| SourceEntry {
            key: file.id,
            name: file.name,
        })
        .collect();
    let request = SyncRequest::ImportSelected(entries);
    let run = syncs
        .start(authz.origin(), path_params.connection_id.as_uuid(), request)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(WorkspaceConnectionSync::from_model(
            run,
            authz.account.clone().into(),
        )),
    ))
}

fn import_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Import selected files")
        .description(
            "Imports the files selected in the provider's picker (file services only). \
             Already-imported files are skipped. Returns the created sync; poll it for completion.",
        )
        .response::<202, Json<WorkspaceConnectionSync>>()
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
        connection_id = %path_params.connection_id,
    )
)]
async fn export_files(
    State(syncs): State<domain::WorkspaceConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    ValidateJson(request): ValidateJson<ExportWorkspaceFiles>,
) -> Result<(StatusCode, Json<WorkspaceConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Exporting selected files to connection");
    let request = SyncRequest::ExportSelected(request.file_ids);
    let run = syncs
        .start(authz.origin(), path_params.connection_id.as_uuid(), request)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(WorkspaceConnectionSync::from_model(
            run,
            authz.account.clone().into(),
        )),
    ))
}

fn export_files_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Export files to connection")
        .description(
            "Exports the selected workspace files to the connection, each as a new provider \
             file. Returns the created sync; poll it for completion.",
        )
        .response::<202, Json<WorkspaceConnectionSync>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Lists sync runs for a connection, most recent first.
#[tracing::instrument(
    skip_all,
    fields(
        connection_id = %path_params.connection_id,
    )
)]
async fn list_connection_syncs(
    State(syncs): State<domain::WorkspaceConnectionSyncService>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<WorkspaceConnectionPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<Json<WorkspaceConnectionSyncsPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing connection syncs");

    let page = syncs
        .list_for_connection(
            authz.workspace.id,
            path_params.connection_id.as_uuid(),
            pagination.into_cursor(),
        )
        .await?;
    Ok(Json(Page::from_cursor_page(page, sync_response)))
}

fn list_connection_syncs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connection syncs")
        .description("Returns the connection's sync history, most recent first.")
        .response::<200, Json<WorkspaceConnectionSyncsPage>>()
}

/// Lists sync runs across every connection in the workspace, most recent first.
///
/// Optional `status` and repeatable `provider` query filters narrow the result
/// (a sync matches if its connection uses any of the given providers). Requires
/// `ViewConnections` permission.
#[tracing::instrument(skip_all)]
async fn list_workspace_syncs(
    State(syncs): State<domain::WorkspaceConnectionSyncService>,
    authz: Authorized<markers::ViewConnections>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<WorkspaceSyncsQuery>,
) -> Result<Json<WorkspaceConnectionSyncsPage>> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace syncs");

    let page = syncs
        .list(
            authz.workspace.id,
            pagination.into_cursor(),
            query.status,
            &query.provider,
        )
        .await?;
    Ok(Json(Page::from_cursor_page(page, sync_response)))
}

fn list_workspace_syncs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace syncs")
        .description(
            "Returns all sync runs across the workspace's connections, most recent first, \
             with optional status and provider filters.",
        )
        .response::<200, Json<WorkspaceConnectionSyncsPage>>()
}

/// Retrieves a single sync run for a connection.
#[tracing::instrument(
    skip_all,
    fields(
        connection_id = %path_params.connection_id,
        sync_id = %path_params.sync_id,
    )
)]
async fn read_connection_sync(
    State(syncs): State<domain::WorkspaceConnectionSyncService>,
    authz: Authorized<markers::ViewConnections>,
    Path(path_params): Path<WorkspaceConnectionSyncPathParams>,
) -> Result<Json<WorkspaceConnectionSync>> {
    tracing::debug!(target: TRACING_TARGET, "Reading connection sync");

    let run = syncs
        .find(
            authz.workspace.id,
            path_params.connection_id.as_uuid(),
            path_params.sync_id,
        )
        .await?;
    Ok(Json(sync_response(run)))
}

fn read_connection_sync_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection sync")
        .description("Returns a single sync run for the connection.")
        .response::<200, Json<WorkspaceConnectionSync>>()
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
        connection_id = %path_params.connection_id,
        sync_id = %path_params.sync_id,
    )
)]
async fn cancel_connection_sync(
    State(syncs): State<domain::WorkspaceConnectionSyncService>,
    authz: Authorized<markers::RunConnectionSyncs>,
    Path(path_params): Path<WorkspaceConnectionSyncPathParams>,
) -> Result<Json<WorkspaceConnectionSync>> {
    tracing::debug!(target: TRACING_TARGET, "Cancelling connection sync");

    let cancelled = syncs
        .cancel(
            authz.workspace.id,
            path_params.connection_id.as_uuid(),
            path_params.sync_id,
        )
        .await?;
    Ok(Json(sync_response(cancelled)))
}

fn cancel_connection_sync_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Cancel connection sync")
        .description("Cancels an in-progress sync run. A run that already finished returns 409.")
        .response::<200, Json<WorkspaceConnectionSync>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Maps a sync run with its trigger to the response.
fn sync_response(run: WithAccountRef<WorkspaceConnectionSyncModel>) -> WorkspaceConnectionSync {
    WorkspaceConnectionSync::from_model(run.item, run.account.into())
}

/// Returns routes for connection sync operations.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/syncs",
            get_with(list_workspace_syncs, list_workspace_syncs_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/sync",
            post_with(sync_connection, sync_connection_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/import",
            post_with(import_files, import_files_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/export",
            post_with(export_files, export_files_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/syncs",
            get_with(list_connection_syncs, list_connection_syncs_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/syncs/{syncId}",
            get_with(read_connection_sync, read_connection_sync_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/connections/{connectionId}/syncs/{syncId}/cancel",
            post_with(cancel_connection_sync, cancel_connection_sync_docs),
        )
        .with_path_items(|item| item.tag("Connection Syncs"))
}
