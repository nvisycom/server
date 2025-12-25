//! Project pipeline management handlers.
//!
//! This module provides comprehensive pipeline management functionality for projects,
//! including creation, listing, updating, and deletion of CI/CD pipelines.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::{CreatePipeline, PipelinePathParams, ProjectPathParams};
use crate::handler::response::{Pipeline, Pipelines};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project pipeline operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_pipelines";

/// Lists all pipelines for a project.
#[tracing::instrument(skip(pg_client))]
async fn list_project_pipelines(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<Pipelines>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewPipelines,
        )
        .await?;

    // For now, return empty list until database schema is implemented
    let pipelines = vec![];

    tracing::debug!(
        target: TRACING_TARGET,
        project_id = %path_params.project_id,
        count = pipelines.len(),
        "project pipelines listed successfully"
    );

    Ok((StatusCode::OK, Json(pipelines)))
}

/// Gets details of a specific pipeline.
#[tracing::instrument(skip(pg_client))]
async fn get_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<PipelinePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<Pipeline>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewPipelines,
        )
        .await?;

    // For now, return not found until database schema is implemented
    Err(ErrorKind::NotFound.with_message("Pipeline not found"))
}

/// Creates a new pipeline for a project.
#[tracing::instrument(skip(pg_client, _request))]
async fn create_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
    Json(_request): Json<CreatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManagePipelines,
        )
        .await?;

    // For now, return not implemented until database schema is implemented
    Err(ErrorKind::InternalServerError.with_message("Pipeline creation not yet implemented"))
}

/// Updates an existing pipeline.
#[tracing::instrument(skip(pg_client, _request))]
async fn update_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<PipelinePathParams>,
    AuthState(auth_claims): AuthState,
    Json(_request): Json<CreatePipeline>,
) -> Result<(StatusCode, Json<Pipeline>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManagePipelines,
        )
        .await?;

    // For now, return not found until database schema is implemented
    Err(ErrorKind::NotFound.with_message("Pipeline not found"))
}

/// Deletes a pipeline.
#[tracing::instrument(skip(pg_client))]
async fn delete_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<PipelinePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManagePipelines,
        )
        .await?;

    // For now, return not found until database schema is implemented
    Err(ErrorKind::NotFound.with_message("Pipeline not found"))
}

/// Returns an [`ApiRouter`] with project pipeline routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/:project_id/pipelines",
            get(list_project_pipelines),
        )
        .api_route(
            "/projects/:project_id/pipelines/:pipeline_id",
            get(get_project_pipeline),
        )
        .api_route(
            "/projects/:project_id/pipelines",
            post(create_project_pipeline),
        )
        .api_route(
            "/projects/:project_id/pipelines/:pipeline_id",
            put(update_project_pipeline),
        )
        .api_route(
            "/projects/:project_id/pipelines/:pipeline_id",
            delete(delete_project_pipeline),
        )
}

