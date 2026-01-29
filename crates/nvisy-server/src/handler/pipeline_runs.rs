//! Pipeline run handlers for viewing execution history.
//!
//! This module provides handlers for listing and retrieving pipeline runs,
//! which represent individual executions of a pipeline.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{WorkspacePipelineRepository, WorkspacePipelineRunRepository};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query};
use crate::handler::request::{CursorPagination, PipelinePathParams, PipelineRunPathParams};
use crate::handler::response::{ErrorResponse, PipelineRun, PipelineRunsPage};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for pipeline run operations.
const TRACING_TARGET: &str = "nvisy_server::handler::pipeline_runs";

/// Lists runs for a specific pipeline.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn list_pipeline_runs(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelinePathParams>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<PipelineRunsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipeline runs");

    let mut conn = pg_client.get_connection().await?;

    let Some(pipeline) = conn
        .find_workspace_pipeline_by_id(path_params.pipeline_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message("Pipeline not found")
            .with_resource("pipeline"));
    };

    auth_state
        .authorize_workspace(&mut conn, pipeline.workspace_id, Permission::ViewPipelines)
        .await?;

    let page = conn
        .cursor_list_workspace_pipeline_runs(path_params.pipeline_id, pagination.into(), None)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        run_count = page.items.len(),
        "Pipeline runs listed"
    );

    Ok((
        StatusCode::OK,
        Json(PipelineRunsPage::from_cursor_page(
            page,
            PipelineRun::from_model,
        )),
    ))
}

fn list_pipeline_runs_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipeline runs")
        .description("Returns all runs for a specific pipeline.")
        .response::<200, Json<PipelineRunsPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Gets a specific pipeline run.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        run_id = %path_params.run_id,
    )
)]
async fn get_pipeline_run(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelineRunPathParams>,
) -> Result<(StatusCode, Json<PipelineRun>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline run");

    let mut conn = pg_client.get_connection().await?;

    let run = conn
        .find_workspace_pipeline_run_by_id(path_params.run_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Pipeline run not found")
                .with_resource("pipeline_run")
        })?;

    // Get workspace_id from the pipeline
    let pipeline = conn
        .find_workspace_pipeline_by_id(run.pipeline_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Pipeline not found")
                .with_resource("pipeline")
        })?;

    auth_state
        .authorize_workspace(&mut conn, pipeline.workspace_id, Permission::ViewPipelines)
        .await?;

    tracing::debug!(target: TRACING_TARGET, "Pipeline run retrieved");

    Ok((StatusCode::OK, Json(PipelineRun::from_model(run))))
}

fn get_pipeline_run_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get pipeline run")
        .description("Returns details for a specific pipeline run.")
        .response::<200, Json<PipelineRun>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all pipeline run routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/pipelines/{pipelineId}/runs/",
            get_with(list_pipeline_runs, list_pipeline_runs_docs),
        )
        .api_route(
            "/pipeline-runs/{runId}/",
            get_with(get_pipeline_run, get_pipeline_run_docs),
        )
        .with_path_items(|item| item.tag("Pipeline Runs"))
}
