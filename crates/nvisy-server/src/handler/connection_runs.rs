//! Workspace connection sync run handlers.
//!
//! Exposes the sync run history for a connection: the record of each
//! synchronization execution, its trigger, progress, and outcome. A run can be
//! triggered manually; background execution that advances a run to completion
//! is handled elsewhere.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewWorkspaceConnectionRun, WorkspaceConnection};
use nvisy_postgres::query::{WorkspaceConnectionRepository, WorkspaceConnectionRunRepository};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, WorkspaceContext};
use crate::handler::request::{
    ConnectionPathParams, ConnectionRunPathParams, ConnectionRunsQuery, CursorPagination,
};
use crate::handler::response::{ConnectionRun, ConnectionRunsPage, ErrorResponse};
use crate::handler::{Error, Result};
use crate::service::ServiceState;

/// Tracing target for workspace connection run operations.
const TRACING_TARGET: &str = "nvisy_server::handler::connection_runs";

/// Triggers a new sync run for a connection.
///
/// Records a manually-triggered run in the `running` state. Requires
/// `ManageConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn create_connection_run(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
) -> Result<(StatusCode, Json<ConnectionRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Triggering connection sync run");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageConnections)
        .await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    let run = conn
        .create_workspace_connection_run(NewWorkspaceConnectionRun {
            connection_id: connection.id,
            account_id: Some(auth_state.account_id),
            ..Default::default()
        })
        .await?;

    tracing::debug!(target: TRACING_TARGET, run_id = %run.id, "Connection sync run triggered");

    Ok((StatusCode::CREATED, Json(ConnectionRun::from_model(run))))
}

fn create_connection_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Trigger connection sync run")
        .description("Records a manually-triggered sync run for the connection.")
        .response::<201, Json<ConnectionRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Lists sync runs for a connection.
///
/// Returns the run history in reverse chronological order. Requires
/// `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_id = %path_params.connection_id,
    )
)]
async fn list_connection_runs(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionPathParams>,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<ConnectionRunsQuery>,
) -> Result<(StatusCode, Json<ConnectionRunsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing connection sync runs");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let connection = find_connection(&mut conn, workspace.id, path_params.connection_id).await?;

    let page = conn
        .cursor_list_workspace_connection_runs(connection.id, pagination.into(), query.status)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Connection sync runs listed",
    );

    Ok((
        StatusCode::OK,
        Json(ConnectionRunsPage::from_cursor_page(
            page,
            ConnectionRun::from_model,
        )),
    ))
}

fn list_connection_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List connection sync runs")
        .description("Returns the connection's sync run history, most recent first.")
        .response::<200, Json<ConnectionRunsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Retrieves a single connection sync run.
///
/// The run's connection (and thus workspace) is derived from the run record.
/// Requires `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        run_id = %path_params.run_id,
    )
)]
async fn read_connection_run(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<ConnectionRunPathParams>,
) -> Result<(StatusCode, Json<ConnectionRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting connection sync run");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let run = conn
        .find_connection_run_in_workspace(workspace.id, path_params.run_id)
        .await?
        .ok_or_else(|| Error::not_found("connection_run"))?;

    tracing::debug!(target: TRACING_TARGET, "Connection sync run retrieved");

    Ok((StatusCode::OK, Json(ConnectionRun::from_model(run))))
}

fn read_connection_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection sync run")
        .description("Returns a single connection sync run by its identifier.")
        .response::<200, Json<ConnectionRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Loads a connection within a workspace, mapping a missing row to not-found.
async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_id: Uuid,
) -> Result<WorkspaceConnection> {
    conn.find_connection_in_workspace(workspace_id, connection_id)
        .await?
        .ok_or_else(|| Error::not_found("connection"))
}

/// Returns routes for workspace connection sync run management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionId}/runs/",
            post_with(create_connection_run, create_connection_run_docs)
                .get_with(list_connection_runs, list_connection_runs_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connection-runs/{runId}/",
            get_with(read_connection_run, read_connection_run_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
