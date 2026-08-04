//! Connection sync handlers: import from and export to a connection.
//!
//! A sync moves a single object between a workspace's external object-store
//! connection and the internal file store. Triggering a sync opens a
//! [`WorkspaceConnectionRun`] and performs the transfer in the background;
//! clients poll the sync detail endpoint for completion.
//!
//! [`WorkspaceConnectionRun`]: nvisy_postgres::model::WorkspaceConnectionRun

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceConnectionRun, WorkspaceConnection};
use nvisy_postgres::query::{
    WorkspaceConnectionRepository, WorkspaceConnectionRunRepository, WorkspaceFileRepository,
};
use nvisy_postgres::types::{ConnectionId, SyncStatus, SyncTriggerType};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, WorkspaceContext,
};
use crate::handler::request::{
    ConnectionPathParams, ConnectionSyncPathParams, CursorPagination, SyncConnection, SyncDirection,
};
use crate::handler::response::{ConnectionSync, ConnectionSyncsPage, ErrorResponse, Page};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{ConnectionSyncService, CryptoService, ServiceState};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connection_syncs";

/// Triggers a sync between the connection and the workspace file store.
///
/// Decrypts the stored credentials, opens a `Manual` sync run, and performs the
/// import or export in the background. Returns `202 Accepted` with the created
/// sync immediately; poll the sync detail endpoint for completion. Requires
/// `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn sync_connection(
    State(pg_client): State<PgClient>,
    State(crypto): State<CryptoService>,
    State(connection_sync): State<ConnectionSyncService>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
    ValidateJson(request): ValidateJson<SyncConnection>,
) -> Result<(StatusCode, Json<ConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Triggering connection sync");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    // Resolve the export target (if any) before starting the run, so a bad
    // request fails fast with a 400/404 rather than a failed run.
    let export_file = match request.direction {
        SyncDirection::Export => {
            let file_id = request.file_id.ok_or_else(|| {
                ErrorKind::BadRequest.with_message("fileId is required to export")
            })?;
            let file = conn
                .find_file_in_workspace(workspace.id, file_id)
                .await?
                .ok_or_else(|| Error::not_found("file"))?;
            Some(file)
        }
        SyncDirection::Import => None,
    };

    let credentials: serde_json::Value =
        crypto.decrypt_json(workspace.id, &connection.encrypted_data)?;

    let new_run = NewWorkspaceConnectionRun {
        connection_id: connection.id,
        account_id: Some(auth_state.account_id),
        trigger_type: Some(SyncTriggerType::Manual),
        status: Some(SyncStatus::Running),
        records_synced: Some(0),
        metadata: None,
    };
    let run = conn.create_workspace_connection_run(new_run).await?;

    // Perform the transfer in the background; the run tracks its outcome. The
    // transfer itself runs in an inner task so that a panic is caught (via the
    // join error) and recorded as a failed run rather than leaving it stuck in
    // `Running`.
    let run_id = run.id;
    let account_id = auth_state.account_id;
    tokio::spawn(async move {
        let transfer = connection_sync.clone();
        let key = request.key;
        let work = tokio::spawn(async move {
            match request.direction {
                SyncDirection::Import => transfer
                    .import_object(&connection, credentials, account_id, &key)
                    .await
                    .map(|_| ()),
                SyncDirection::Export => {
                    let file = export_file.expect("export file resolved above");
                    transfer
                        .export_file(&connection, credentials, &file, &key)
                        .await
                }
            }
        });

        let result = match work.await {
            Ok(result) => result,
            Err(join_err) => Err(ErrorKind::InternalServerError
                .with_message("Sync task terminated unexpectedly")
                .with_context(join_err.to_string())),
        };
        connection_sync.finish_run(run_id, result).await;
    });

    Ok((StatusCode::ACCEPTED, Json(ConnectionSync::from_model(run))))
}

fn sync_connection_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Sync connection")
        .description("Imports an object from or exports a file to the connection. Returns the created sync; poll it for completion.")
        .response::<202, Json<ConnectionSync>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists sync runs for a connection, most recent first.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn list_connection_syncs(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<ConnectionSyncsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing connection syncs");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    let page = conn
        .cursor_list_workspace_connection_runs(connection.id, pagination.into(), None)
        .await?;

    let page = Page::from_cursor_page(page, |(run, _creator)| ConnectionSync::from_model(run));

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

/// Retrieves a single sync run for a connection.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
        sync_id = %path_params.sync_id,
    )
)]
async fn read_connection_sync(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionSyncPathParams>,
) -> Result<(StatusCode, Json<ConnectionSync>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading connection sync");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    // Confirm the connection is in this workspace before exposing its run.
    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    let run = conn
        .find_connection_run_in_workspace(workspace.id, path_params.sync_id)
        .await?
        .filter(|run| run.connection_id == connection.id)
        .ok_or_else(|| Error::not_found("connection_sync"))?;

    Ok((StatusCode::OK, Json(ConnectionSync::from_model(run))))
}

fn read_connection_sync_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection sync")
        .description("Returns a single sync run for the connection.")
        .response::<200, Json<ConnectionSync>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
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
            "/workspaces/{workspaceSlug}/connections/{connectionId}/sync/",
            post_with(sync_connection, sync_connection_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/syncs/",
            get_with(list_connection_syncs, list_connection_syncs_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/syncs/{syncId}/",
            get_with(read_connection_sync, read_connection_sync_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
