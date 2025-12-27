//! Project template management handlers.
//!
//! This module provides comprehensive template management functionality for projects,
//! including creation, listing, updating, and deletion of project templates.
//! Currently a stub implementation pending database schema.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission};
use crate::handler::request::{CreateTemplate, ProjectPathParams, TemplatePathParams};
use crate::handler::response::{Template, Templates};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project template operations.
const TRACING_TARGET: &str = "nvisy_server::handler::templates";

/// Lists all templates for a project.
///
/// Returns all configured templates. Requires `ViewTemplates` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn list_project_templates(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<Templates>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing project templates");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewTemplates,
        )
        .await?;

    // Stub: return empty list until database schema is implemented
    let templates = vec![];

    tracing::debug!(
        target: TRACING_TARGET,
        count = templates.len(),
        "Project templates listed successfully",
    );

    Ok((StatusCode::OK, Json(templates)))
}

/// Gets details of a specific template.
///
/// Returns template configuration. Requires `ViewTemplates` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        template_id = %path_params.template_id,
    )
)]
async fn get_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<TemplatePathParams>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<Template>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading project template");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ViewTemplates,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Template not found")
        .with_resource("template"))
}

/// Creates a new template for a project.
///
/// Creates a reusable template. Requires `ManageTemplates` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn create_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_state): AuthState,
    Json(_request): Json<CreateTemplate>,
) -> Result<(StatusCode, Json<Template>)> {
    tracing::info!(target: TRACING_TARGET, "Creating project template");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // Stub: return not implemented until database schema is implemented
    Err(ErrorKind::InternalServerError
        .with_message("Template creation not yet implemented")
        .with_resource("template"))
}

/// Updates an existing template.
///
/// Updates template configuration. Requires `ManageTemplates` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        template_id = %path_params.template_id,
    )
)]
async fn update_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<TemplatePathParams>,
    AuthState(auth_state): AuthState,
    Json(_request): Json<CreateTemplate>,
) -> Result<(StatusCode, Json<Template>)> {
    tracing::info!(target: TRACING_TARGET, "Updating project template");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Template not found")
        .with_resource("template"))
}

/// Deletes a template.
///
/// Permanently removes the template. Requires `ManageTemplates` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
        template_id = %path_params.template_id,
    )
)]
async fn delete_project_template(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<TemplatePathParams>,
    AuthState(auth_state): AuthState,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Deleting project template");

    auth_state
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Template not found")
        .with_resource("template"))
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
