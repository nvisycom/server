//! Integration run management handlers.
//!
//! This module provides handlers for viewing integration run history and status.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::http::StatusCode;
use nvisy_postgres::query::WorkspaceIntegrationRunRepository;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, Query};
use crate::handler::request::{IntegrationRunPathParams, Pagination, WorkspacePathParams};
use crate::handler::response::{ErrorResponse, IntegrationRun, IntegrationRuns};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for integration run operations.
const TRACING_TARGET: &str = "nvisy_server::handler::integration_runs";

/// Lists integration runs for a workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_workspace_runs(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(pagination): Query<Pagination>,
) -> Result<(StatusCode, Json<IntegrationRuns>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace integration runs");

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewIntegrations,
        )
        .await?;

    let runs = conn
        .find_runs_by_workspace(path_params.workspace_id, pagination.into())
        .await?;

    let runs: IntegrationRuns = IntegrationRun::from_models(runs);

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = runs.len(),
        "Workspace integration runs listed"
    );

    Ok((StatusCode::OK, Json(runs)))
}

fn list_workspace_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace integration runs")
        .description("Returns all integration runs for a workspace.")
        .response::<200, Json<IntegrationRuns>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Gets a specific integration run.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        run_id = %path_params.run_id,
    )
)]
async fn get_run(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<IntegrationRunPathParams>,
) -> Result<(StatusCode, Json<IntegrationRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting integration run");

    let run = conn
        .find_run_by_id(path_params.run_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Integration run not found")
                .with_resource("integration_run")
        })?;

    auth_state
        .authorize_workspace(&mut conn, run.workspace_id, Permission::ViewIntegrations)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Integration run retrieved");

    Ok((StatusCode::OK, Json(IntegrationRun::from_model(run))))
}

fn get_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get integration run")
        .description("Returns details for a specific integration run.")
        .response::<200, Json<IntegrationRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns routes for integration run management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspace_id}/runs/",
            get_with(list_workspace_runs, list_workspace_runs_docs),
        )
        .api_route("/runs/{run_id}", get_with(get_run, get_run_docs))
        .with_path_items(|item| item.tag("Integration Runs"))
}
