//! Project template management handlers.
//!
//! This module provides comprehensive template management functionality for projects,
//! including creation, listing, updating, and deletion of project templates.
//! Currently a stub implementation pending database schema.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::http::StatusCode;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool};
use crate::handler::request::{CreateTemplate, ProjectPathParams, TemplatePathParams};
use crate::handler::response::{ErrorResponse, Template, Templates};
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
    PgPool(mut conn): PgPool,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<Templates>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing project templates");

    auth_state
        .authorize_project(&mut conn, path_params.project_id, Permission::ViewTemplates)
        .await?;

    // Stub: return empty list until database schema is implemented
    let templates = vec![];

    tracing::debug!(
        target: TRACING_TARGET,
        count = templates.len(),
        "Project templates listed ",
    );

    Ok((StatusCode::OK, Json(templates)))
}

fn list_project_templates_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List templates")
        .description("Returns all configured templates for the project.")
        .response::<200, Json<Templates>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
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
    PgPool(mut conn): PgPool,
    Path(path_params): Path<TemplatePathParams>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<Template>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading project template");

    auth_state
        .authorize_project(&mut conn, path_params.project_id, Permission::ViewTemplates)
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Template not found")
        .with_resource("template"))
}

fn get_project_template_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get template")
        .description("Returns details of a specific template configuration.")
        .response::<200, Json<Template>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
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
    PgPool(mut conn): PgPool,
    Path(path_params): Path<ProjectPathParams>,
    AuthState(auth_state): AuthState,
    Json(_request): Json<CreateTemplate>,
) -> Result<(StatusCode, Json<Template>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating project template");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // Stub: return not implemented until database schema is implemented
    Err(ErrorKind::InternalServerError
        .with_message("Template creation not yet implemented")
        .with_resource("template"))
}

fn create_project_template_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create template")
        .description("Creates a new reusable template for the project.")
        .response::<201, Json<Template>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
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
    PgPool(mut conn): PgPool,
    Path(path_params): Path<TemplatePathParams>,
    AuthState(auth_state): AuthState,
    Json(_request): Json<CreateTemplate>,
) -> Result<(StatusCode, Json<Template>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating project template");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Template not found")
        .with_resource("template"))
}

fn update_project_template_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update template")
        .description("Updates an existing template configuration.")
        .response::<200, Json<Template>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
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
    PgPool(mut conn): PgPool,
    Path(path_params): Path<TemplatePathParams>,
    AuthState(auth_state): AuthState,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting project template");

    auth_state
        .authorize_project(
            &mut conn,
            path_params.project_id,
            Permission::ManageTemplates,
        )
        .await?;

    // Stub: return not found until database schema is implemented
    Err(ErrorKind::NotFound
        .with_message("Template not found")
        .with_resource("template"))
}

fn delete_project_template_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete template")
        .description("Permanently removes a template from the project.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns an [`ApiRouter`] with project template routes.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/{project_id}/templates",
            get_with(list_project_templates, list_project_templates_docs)
                .post_with(create_project_template, create_project_template_docs),
        )
        .api_route(
            "/projects/{project_id}/templates/{template_id}",
            get_with(get_project_template, get_project_template_docs)
                .put_with(update_project_template, update_project_template_docs)
                .delete_with(delete_project_template, delete_project_template_docs),
        )
        .with_path_items(|item| item.tag("Templates"))
}
