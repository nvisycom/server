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
use nvisy_postgres::model::{
    NewWorkspaceConnectionRun, WorkspaceConnection, WorkspaceConnectionRun,
};
use nvisy_postgres::query::{WorkspaceConnectionRepository, WorkspaceConnectionRunRepository};
use nvisy_postgres::types::Username;
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
        connection_slug = %path_params.connection_slug,
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

    let connection = find_connection(&mut conn, workspace.id, &path_params.connection_slug).await?;

    let run = conn
        .create_workspace_connection_run(NewWorkspaceConnectionRun {
            connection_id: connection.id,
            account_id: Some(auth_state.account_id),
            ..Default::default()
        })
        .await?;

    tracing::debug!(target: TRACING_TARGET, run_number = run.run_number, "Connection sync run triggered");

    let (_, run, trigger_username) =
        find_connection_run(&mut conn, workspace.id, connection.slug.as_str(), run.run_number)
            .await?;

    Ok((
        StatusCode::CREATED,
        Json(ConnectionRun::from_model(
            run,
            connection.slug,
            workspace.slug,
            trigger_username,
        )),
    ))
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
        connection_slug = %path_params.connection_slug,
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

    let connection = find_connection(&mut conn, workspace.id, &path_params.connection_slug).await?;

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
            |(run, trigger_username)| {
                ConnectionRun::from_model(
                    run,
                    connection.slug.clone(),
                    workspace.slug.clone(),
                    trigger_username,
                )
            },
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

/// Lists all sync runs across the workspace's connections.
///
/// Aggregates runs from every connection in the workspace, most recent first,
/// with an optional status filter. Requires `ViewConnections`.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn list_workspace_connection_runs(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(pagination): Query<CursorPagination>,
    Query(query): Query<ConnectionRunsQuery>,
) -> Result<(StatusCode, Json<ConnectionRunsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace connection sync runs");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewConnections)
        .await?;

    let page = conn
        .cursor_list_workspace_connection_runs_all(workspace.id, pagination.into(), query.status)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Workspace connection sync runs listed",
    );

    Ok((
        StatusCode::OK,
        Json(ConnectionRunsPage::from_cursor_page(
            page,
            |(run, connection_slug, trigger_username)| {
                ConnectionRun::from_model(
                    run,
                    connection_slug,
                    workspace.slug.clone(),
                    trigger_username,
                )
            },
        )),
    ))
}

fn list_workspace_connection_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace connection sync runs")
        .description(
            "Returns all sync runs across the workspace's connections, most \
             recent first, with an optional status filter.",
        )
        .response::<200, Json<ConnectionRunsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Retrieves a single connection sync run.
///
/// The run is addressed as `(connection slug, run number)` within the
/// workspace. Requires `ViewConnections` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        connection_slug = %path_params.connection_slug,
        run_number = path_params.run_number,
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

    let (connection, run, trigger_username) = find_connection_run(
        &mut conn,
        workspace.id,
        &path_params.connection_slug,
        path_params.run_number,
    )
    .await?;

    tracing::debug!(target: TRACING_TARGET, "Connection sync run retrieved");

    Ok((
        StatusCode::OK,
        Json(ConnectionRun::from_model(
            run,
            connection.slug,
            workspace.slug,
            trigger_username,
        )),
    ))
}

fn read_connection_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get connection sync run")
        .description("Returns a single connection sync run by its number.")
        .response::<200, Json<ConnectionRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Loads a connection within a workspace by slug, mapping a missing row to
/// not-found.
async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_slug: &str,
) -> Result<WorkspaceConnection> {
    conn.find_connection_in_workspace_by_slug(workspace_id, connection_slug)
        .await?
        .map(|(connection, _)| connection)
        .ok_or_else(|| Error::not_found("connection"))
}

/// Resolves a run addressed as `(connection slug, run number)` within a
/// workspace.
///
/// Returns both the owning connection and the run, or NotFound if either the
/// connection slug or the run number does not resolve.
async fn find_connection_run(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_slug: &str,
    run_number: i32,
) -> Result<(WorkspaceConnection, WorkspaceConnectionRun, Option<Username>)> {
    let connection = find_connection(conn, workspace_id, connection_slug).await?;
    let (run, trigger_username) = conn
        .find_connection_run_by_number(connection.id, run_number)
        .await?
        .ok_or_else(|| Error::not_found("connection_run"))?;
    Ok((connection, run, trigger_username))
}

/// Returns routes for workspace connection sync run management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/connections/runs/",
            get_with(
                list_workspace_connection_runs,
                list_workspace_connection_runs_docs,
            ),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionSlug}/runs/",
            post_with(create_connection_run, create_connection_run_docs)
                .get_with(list_connection_runs, list_connection_runs_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/connections/{connectionSlug}/runs/{runNumber}/",
            get_with(read_connection_run, read_connection_run_docs),
        )
        .with_path_items(|item| item.tag("Connections"))
}
