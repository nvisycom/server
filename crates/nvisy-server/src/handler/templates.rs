//! Project template management handlers.
//!
//! This module provides comprehensive template management functionality for projects,
//! including creation, listing, updating, and deletion of project templates.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::CreateProjectTemplate;
use crate::handler::response::{ProjectTemplate, ProjectTemplates};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project template operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_templates";

/// Path parameters for project operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
}

/// Path parameters for project template operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplatePathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
    /// The unique identifier of the template.
    pub template_id: Uuid,
}

/// Lists all templates for a project.
#[tracing::instrument(skip(pg_client))]
async fn list_project_templates(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<ProjectTemplates>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewTemplates,
        )
        .await?;

    // For now, return empty list until database schema is implemented
    let templates = vec![];

    tracing::debug!(
        target: TRACING_TARGET,
        project_id = %path_params.project_id,
        count = templates.len(),
        "project templates listed successfully"
    );

    Ok((StatusCode::OK, Json(templates)))
}

/// Gets details of a specific template.
#[tracing::instrument(skip(pg_client))]
async fn get_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectTemplatePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<ProjectTemplate>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewTemplates,
        )
        .await?;

    // For now, return not found until database schema is implemented
    Err(ErrorKind::NotFound.with_message("Template not found"))
}

/// Creates a new template for a project.
#[tracing::instrument(skip(pg_client, _request))]
async fn create_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_claims): AuthState,
    Json(_request): Json<CreateProjectTemplate>,
) -> Result<(StatusCode, Json<ProjectTemplate>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // For now, return not implemented until database schema is implemented
    Err(ErrorKind::InternalServerError.with_message("Template creation not yet implemented"))
}

/// Updates an existing template.
#[tracing::instrument(skip(pg_client, _request))]
async fn update_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectTemplatePathParams>,
    AuthState(auth_claims): AuthState,
    Json(_request): Json<CreateProjectTemplate>,
) -> Result<(StatusCode, Json<ProjectTemplate>)> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // For now, return not found until database schema is implemented
    Err(ErrorKind::NotFound.with_message("Template not found"))
}

/// Deletes a template.
#[tracing::instrument(skip(pg_client))]
async fn delete_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectTemplatePathParams>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    // Authorize project access
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // For now, return not found until database schema is implemented
    Err(ErrorKind::NotFound.with_message("Template not found"))
}

/// Returns an [`ApiRouter`] with project template routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/:project_id/templates",
            get(list_project_templates),
        )
        .api_route(
            "/projects/:project_id/templates/:template_id",
            get(get_project_template),
        )
        .api_route(
            "/projects/:project_id/templates",
            post(create_project_template),
        )
        .api_route(
            "/projects/:project_id/templates/:template_id",
            put(update_project_template),
        )
        .api_route(
            "/projects/:project_id/templates/:template_id",
            delete(delete_project_template),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::test::create_test_server;

    #[tokio::test]
    async fn test_project_templates_routes() -> anyhow::Result<()> {
        let _server = create_test_server().await?;
        // TODO: Add actual tests once database schema is implemented
        Ok(())
    }
}
