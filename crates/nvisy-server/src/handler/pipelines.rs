//! Pipeline management handlers for CRUD operations.
//!
//! This module provides comprehensive pipeline management functionality including
//! creating, reading, updating, deleting pipelines, and listing pipelines within
//! a workspace. All operations are secured with role-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{
    WorkspacePipelineArtifactRepository, WorkspacePipelineRepository,
    WorkspacePipelineRunRepository,
};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    CreatePipeline, CursorPagination, PipelineFilter, PipelinePathParams, PipelineRunPathParams,
    UpdatePipeline, WorkspacePathParams,
};
use crate::handler::response::{
    ErrorResponse, Page, Pipeline, PipelineRun, PipelineRunsPage, PipelineSummary,
};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for pipeline operations.
const TRACING_TARGET: &str = "nvisy_server::handler::pipelines";

/// Creates a new pipeline within a workspace.
///
/// The creator is automatically set as the owner of the pipeline.
/// Requires `UploadFiles` permission for the workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn create_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<CreatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating pipeline");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::CreatePipelines,
        )
        .await?;

    let new_pipeline = request.into_model(path_params.workspace_id, auth_state.account_id);
    let pipeline = conn.create_workspace_pipeline(new_pipeline).await?;

    let response = Pipeline::from_model(pipeline);

    tracing::info!(
        target: TRACING_TARGET,
        pipeline_id = %response.pipeline_id,
        "Pipeline created",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create pipeline")
        .description("Creates a new pipeline in the workspace. The creator is set as the owner.")
        .response::<201, Json<Pipeline>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Lists all pipelines in a workspace with optional filtering.
///
/// Supports filtering by status and searching by name.
/// Requires `ViewFiles` permission for the workspace.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_pipelines(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(pagination): Query<CursorPagination>,
    Query(filter): Query<PipelineFilter>,
) -> Result<(StatusCode, Json<Page<PipelineSummary>>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing pipelines");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::ViewPipelines,
        )
        .await?;

    let page = conn
        .cursor_list_workspace_pipelines(
            path_params.workspace_id,
            pagination.into(),
            filter.status,
            filter.search.as_deref(),
        )
        .await?;

    let response = Page::from_cursor_page(page, PipelineSummary::from_model);

    tracing::debug!(
        target: TRACING_TARGET,
        pipeline_count = response.items.len(),
        "Pipelines listed",
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_pipelines_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pipelines")
        .description("Returns all pipelines in the workspace with optional filtering by status and name search.")
        .response::<200, Json<Page<PipelineSummary>>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Retrieves a pipeline by ID.
///
/// The workspace is derived from the pipeline record for authorization.
/// Returns the pipeline with all artifacts from its runs.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn get_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelinePathParams>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Getting pipeline");

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

    // Fetch artifacts for all runs of this pipeline
    let artifacts = conn
        .list_workspace_pipeline_artifacts(path_params.pipeline_id)
        .await?;

    let response = Pipeline::from_model_with_artifacts(pipeline, artifacts);

    tracing::info!(target: TRACING_TARGET, "Pipeline retrieved");

    Ok((StatusCode::OK, Json(response)))
}

fn get_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get pipeline")
        .description("Returns a pipeline by its unique identifier.")
        .response::<200, Json<Pipeline>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates an existing pipeline.
///
/// Only the pipeline owner or users with `UpdateFiles` permission can update.
/// Only provided fields are updated.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn update_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelinePathParams>,
    ValidateJson(request): ValidateJson<UpdatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating pipeline");

    let mut conn = pg_client.get_connection().await?;

    let Some(existing) = conn
        .find_workspace_pipeline_by_id(path_params.pipeline_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_message("Pipeline not found")
            .with_resource("pipeline"));
    };

    auth_state
        .authorize_workspace(
            &mut conn,
            existing.workspace_id,
            Permission::UpdatePipelines,
        )
        .await?;

    let update_data = request.into_model();
    let pipeline = conn
        .update_workspace_pipeline(path_params.pipeline_id, update_data)
        .await?;

    let response = Pipeline::from_model(pipeline);

    tracing::info!(target: TRACING_TARGET, "Pipeline updated");

    Ok((StatusCode::OK, Json(response)))
}

fn update_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update pipeline")
        .description("Updates an existing pipeline. Only provided fields are updated.")
        .response::<200, Json<Pipeline>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Soft-deletes a pipeline.
///
/// Requires `DeleteFiles` permission. The pipeline is marked as deleted
/// but data is retained for potential recovery.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn delete_pipeline(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<PipelinePathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting pipeline");

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
        .authorize_workspace(
            &mut conn,
            pipeline.workspace_id,
            Permission::DeletePipelines,
        )
        .await?;

    conn.delete_workspace_pipeline(path_params.pipeline_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Pipeline deleted");

    Ok(StatusCode::OK)
}

fn delete_pipeline_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete pipeline")
        .description("Soft-deletes a pipeline. Data is retained for potential recovery.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

// =============================================================================
// Pipeline Run Handlers
// =============================================================================

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

/// Returns a [`Router`] with all pipeline-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes for listing and creating
        .api_route(
            "/workspaces/{workspaceId}/pipelines/",
            post_with(create_pipeline, create_pipeline_docs)
                .get_with(list_pipelines, list_pipelines_docs),
        )
        // Pipeline operations by ID
        .api_route(
            "/pipelines/{pipelineId}/",
            get_with(get_pipeline, get_pipeline_docs)
                .patch_with(update_pipeline, update_pipeline_docs)
                .delete_with(delete_pipeline, delete_pipeline_docs),
        )
        // Pipeline runs
        .api_route(
            "/pipelines/{pipelineId}/runs/",
            get_with(list_pipeline_runs, list_pipeline_runs_docs),
        )
        .api_route(
            "/pipeline-runs/{runId}/",
            get_with(get_pipeline_run, get_pipeline_run_docs),
        )
        .with_path_items(|item| item.tag("Pipelines"))
}
