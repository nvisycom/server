//! Project pipeline management handlers.
//!
//! This module provides comprehensive pipeline management functionality for projects,
//! including creation, listing, updating, and deletion of CI/CD pipelines.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::CreateProjectPipeline;
use crate::handler::response::{ProjectPipeline, ProjectPipelines};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project pipeline operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_pipelines";

/// Path parameters for project operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
}

/// Path parameters for project pipeline operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPipelinePathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
    /// The unique identifier of the pipeline.
    pub pipeline_id: Uuid,
}

/// Lists all pipelines for a project.
#[tracing::instrument(skip(pg_client))]
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
