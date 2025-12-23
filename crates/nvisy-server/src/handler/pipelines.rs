//! Project pipeline management handlers.
//!
//! This module provides comprehensive pipeline management functionality for projects,
//! including creation, listing, updating, and deletion of CI/CD pipelines.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::CreateProjectPipeline;
use crate::handler::response::{ProjectPipeline, ProjectPipelines};
use crate::handler::{ErrorKind, ErrorResponse, Result};
use crate::service::ServiceState;

/// Tracing target for project pipeline operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_pipelines";

/// Path parameters for project operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
}

/// Path parameters for project pipeline operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPipelinePathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
    /// The unique identifier of the pipeline.
    pub pipeline_id: Uuid,
}

/// Lists all pipelines for a project.
#[tracing::instrument(skip(pg_client))]
#[utoipa::path(
    get, path = "/projects/{projectId}/pipelines", tag = "projects",
    params(ProjectPathParams),
    responses(
        (
            status = OK,
            description = "Project pipelines retrieved successfully",
            body = ProjectPipelines,
        ),
        (
            status = NOT_FOUND,
            description = "Project not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
    ),
)]
async fn list_project_pipelines(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<ProjectPipelines>)> {
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
#[utoipa::path(
    get, path = "/projects/{projectId}/pipelines/{pipelineId}", tag = "projects",
    params(ProjectPipelinePathParams),
    responses(
        (
            status = OK,
            description = "Pipeline details retrieved successfully",
            body = ProjectPipeline,
        ),
        (
            status = NOT_FOUND,
            description = "Pipeline not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
    ),
)]
async fn get_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPipelinePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<ProjectPipeline>)> {
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
#[utoipa::path(
    post, path = "/projects/{projectId}/pipelines", tag = "projects",
    params(ProjectPathParams),
    request_body = CreateProjectPipeline,
    responses(
        (
            status = CREATED,
            description = "Pipeline created successfully",
            body = ProjectPipeline,
        ),
        (
            status = BAD_REQUEST,
            description = "Invalid pipeline data",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Project not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = CONFLICT,
            description = "Pipeline name already exists",
            body = ErrorResponse,
        ),
    ),
)]
async fn create_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
    Json(_request): Json<CreateProjectPipeline>,
) -> Result<(StatusCode, Json<ProjectPipeline>)> {
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
#[utoipa::path(
    put, path = "/projects/{projectId}/pipelines/{pipelineId}", tag = "projects",
    params(ProjectPipelinePathParams),
    request_body = CreateProjectPipeline,
    responses(
        (
            status = OK,
            description = "Pipeline updated successfully",
            body = ProjectPipeline,
        ),
        (
            status = BAD_REQUEST,
            description = "Invalid pipeline data",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Pipeline not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = CONFLICT,
            description = "Pipeline name already exists",
            body = ErrorResponse,
        ),
    ),
)]
async fn update_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPipelinePathParams>,
    AuthState(auth_claims): AuthState,
    Json(_request): Json<CreateProjectPipeline>,
) -> Result<(StatusCode, Json<ProjectPipeline>)> {
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
#[utoipa::path(
    delete, path = "/projects/{projectId}/pipelines/{pipelineId}", tag = "projects",
    params(ProjectPipelinePathParams),
    responses(
        (
            status = NO_CONTENT,
            description = "Pipeline deleted successfully",
        ),
        (
            status = NOT_FOUND,
            description = "Pipeline not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
    ),
)]
async fn delete_project_pipeline(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPipelinePathParams>,
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

/// Returns an [`OpenApiRouter`] with project pipeline routes.
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(
        list_project_pipelines,
        get_project_pipeline,
        create_project_pipeline,
        update_project_pipeline,
        delete_project_pipeline
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::test::create_test_server;

    #[tokio::test]
    async fn test_project_pipelines_routes() -> anyhow::Result<()> {
        let _server = create_test_server().await?;
        // TODO: Add actual tests once database schema is implemented
        Ok(())
    }
}
