//! Project template management handlers.
//!
//! This module provides comprehensive template management functionality for projects,
//! including creation, listing, updating, and deletion of project templates.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::CreateProjectTemplate;
use crate::handler::response::{ProjectTemplate, ProjectTemplates};
use crate::handler::{ErrorKind, ErrorResponse, Result};
use crate::service::ServiceState;

/// Tracing target for project template operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_templates";

/// Path parameters for project operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
}

/// Path parameters for project template operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplatePathParams {
    /// The unique identifier of the project.
    pub project_id: Uuid,
    /// The unique identifier of the template.
    pub template_id: Uuid,
}

/// Lists all templates for a project.
#[tracing::instrument(skip(pg_client))]
#[utoipa::path(
    get, path = "/projects/{projectId}/templates", tag = "projects",
    params(ProjectPathParams),
    responses(
        (
            status = OK,
            description = "Project templates retrieved successfully",
            body = ProjectTemplates,
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
#[utoipa::path(
    get, path = "/projects/{projectId}/templates/{templateId}", tag = "projects",
    params(ProjectTemplatePathParams),
    responses(
        (
            status = OK,
            description = "Template details retrieved successfully",
            body = ProjectTemplate,
        ),
        (
            status = NOT_FOUND,
            description = "Template not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
    ),
)]
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
#[utoipa::path(
    post, path = "/projects/{projectId}/templates", tag = "projects",
    params(ProjectPathParams),
    request_body = CreateProjectTemplate,
    responses(
        (
            status = CREATED,
            description = "Template created successfully",
            body = ProjectTemplate,
        ),
        (
            status = BAD_REQUEST,
            description = "Invalid template data",
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
            description = "Template name already exists",
            body = ErrorResponse,
        ),
    ),
)]
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
#[utoipa::path(
    put, path = "/projects/{projectId}/templates/{templateId}", tag = "projects",
    params(ProjectTemplatePathParams),
    request_body = CreateProjectTemplate,
    responses(
        (
            status = OK,
            description = "Template updated successfully",
            body = ProjectTemplate,
        ),
        (
            status = BAD_REQUEST,
            description = "Invalid template data",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Template not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = CONFLICT,
            description = "Template name already exists",
            body = ErrorResponse,
        ),
    ),
)]
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
#[utoipa::path(
    delete, path = "/projects/{projectId}/templates/{templateId}", tag = "projects",
    params(ProjectTemplatePathParams),
    responses(
        (
            status = NO_CONTENT,
            description = "Template deleted successfully",
        ),
        (
            status = NOT_FOUND,
            description = "Template not found",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Insufficient permissions",
            body = ErrorResponse,
        ),
    ),
)]
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

/// Returns an [`OpenApiRouter`] with project template routes.
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(
        list_project_templates,
        get_project_template,
        create_project_template,
        update_project_template,
        delete_project_template
    ))
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
