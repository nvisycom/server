//! Project pipeline management handlers.
//!
//! This module provides comprehensive pipeline management functionality for projects,
//! including creation, listing, updating, and deletion of processing pipelines.
//! Currently a stub implementation pending database schema.

use aide::axum::ApiRouter;
use axum::http::StatusCode;


use crate::extract::{PgPool, AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::{CreatePipeline, PipelinePathParams, ProjectPathParams};
use crate::handler::response::{Pipeline, Pipelines};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project pipeline operations.
const TRACING_TARGET: &str = "nvisy_server::handler::pipelines";

/// Lists all pipelines for a project.
///
/// Returns all configured pipelines. Requires `ViewPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn list_project_pipelines(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<Pipelines>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing project pipelines");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ViewPipelines,
        )
        .await?;

    // Stub: return empty list until database schema is implemented
    let pipelines = vec![];

    tracing::debug!(
        target: TRACING_TARGET,
        count = pipelines.len(),
        "Project pipelines listed successfully",
    );

    Ok((StatusCode::OK, Json(pipelines)))
}

/// Gets details of a specific pipeline.
///
/// Returns pipeline configuration. Requires `ViewPipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn get_project_pipeline(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<PipelinePathParams>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading project pipeline");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ViewPipelines,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Pipeline not found")
        .with_resource("pipeline"))
}

/// Creates a new pipeline for a project.
///
/// Creates a processing pipeline configuration. Requires `ManagePipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn create_project_pipeline(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_state): AuthState,
    Json(_request): Json<CreatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::info!(target: TRACING_TARGET, "Creating project pipeline");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManagePipelines,
        )
        .await?;

    // Stub: return not implemented until database schema is implemented
    Err(ErrorKind::InternalServerError
        .with_message("Pipeline creation not yet implemented")
        .with_resource("pipeline"))
}

/// Updates an existing pipeline.
///
/// Updates pipeline configuration. Requires `ManagePipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn update_project_pipeline(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<PipelinePathParams>,
    AuthState(auth_state): AuthState,
    Json(_request): Json<CreatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    tracing::info!(target: TRACING_TARGET, "Updating project pipeline");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManagePipelines,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Pipeline not found")
        .with_resource("pipeline"))
}

/// Deletes a pipeline.
///
/// Permanently removes the pipeline. Requires `ManagePipelines` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        pipeline_id = %path_params.pipeline_id,
    )
)]
async fn delete_project_pipeline(
    PgPool(mut conn): PgPool,
    Path(path_params): Path<PipelinePathParams>,
    AuthState(auth_state): AuthState,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Deleting project pipeline");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManagePipelines,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Pipeline not found")
        .with_resource("pipeline"))
}

/// Returns an [`ApiRouter`] with project pipeline routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/{project_id}/pipelines",
            get(list_project_pipelines),
        )
        .api_route(
            "/projects/{project_id}/pipelines/{pipeline_id}",
            get(get_project_pipeline),
        )
        .api_route(
            "/projects/{project_id}/pipelines",
            post(create_project_pipeline),
        )
        .api_route(
            "/projects/{project_id}/pipelines/{pipeline_id}",
            put(update_project_pipeline),
        )
        .api_route(
            "/projects/{project_id}/pipelines/{pipeline_id}",
            delete(delete_project_pipeline),
        )
}
