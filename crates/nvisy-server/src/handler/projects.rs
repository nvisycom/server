//! Project management handlers for CRUD operations.
//!
//! This module provides comprehensive project management functionality including
//! creating, reading, updating, and deleting projects. All operations are secured
//! with role-based access control.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::http::StatusCode;
use nvisy_postgres::PgError;
use nvisy_postgres::model::{NewProjectMember, Project as ProjectModel, ProjectMember};
use nvisy_postgres::query::{ProjectMemberRepository, ProjectRepository};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{CreateProject, Pagination, ProjectPathParams, UpdateProject};
use crate::handler::response::{ErrorResponse, Project, Projects};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project operations.
const TRACING_TARGET: &str = "nvisy_server::handler::projects";

/// Creates a new project with the authenticated user as admin.
///
/// The creator is automatically added as an admin member of the project,
/// granting full management permissions.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_project(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    ValidateJson(request): ValidateJson<CreateProject>,
) -> Result<(StatusCode, Json<Project>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating project");

    let new_project = request.into_model(auth_state.account_id);
    let creator_id = auth_state.account_id;

    let (project, _membership) = conn
        .transaction(|conn| {
            Box::pin(async move {
                let project = conn.create_project(new_project).await?;
                let new_member = NewProjectMember::new_owner(project.id, creator_id);
                let member = conn.add_project_member(new_member).await?;
                Ok::<(ProjectModel, ProjectMember), PgError>((project, member))
            })
        })
        .await?;

    let response = Project::from_model(project);

    tracing::info!(
        target: TRACING_TARGET,
        project_id = %response.project_id,
        "Project created",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_project_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create project")
        .description(
            "Creates a new project. The creator is automatically added as an admin member.",
        )
        .response::<201, Json<Project>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Lists all projects the authenticated user is a member of.
///
/// Returns projects with membership details including the user's role
/// in each project.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn list_projects(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Projects>)> {
    let project_memberships = conn
        .list_user_projects_with_details(auth_state.account_id, pagination.into())
        .await?;

    let projects: Projects = project_memberships
        .into_iter()
        .map(|(project, membership)| Project::from_model_with_membership(project, membership))
        .collect();

    tracing::info!(
        target: TRACING_TARGET,
        project_count = projects.len(),
        "Projects listed",
    );

    Ok((StatusCode::OK, Json(projects)))
}

fn list_projects_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List projects")
        .description("Returns all projects the authenticated user is a member of.")
        .response::<200, Json<Projects>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Retrieves details for a specific project.
///
/// Requires `ViewProject` permission for the requested project.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn read_project(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<Project>)> {
    auth_state
        .authorize_project(&mut conn, path_params.project_id, Permission::ViewProject)
        .await?;

    let Some(project) = conn.find_project_by_id(path_params.project_id).await? else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Project not found: {}", path_params.project_id))
            .with_resource("project"));
    };

    tracing::info!(target: TRACING_TARGET, "Project read");

    let project = Project::from_model(project);
    Ok((StatusCode::OK, Json(project)))
}

fn read_project_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get project")
        .description("Returns details for a specific project.")
        .response::<200, Json<Project>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates an existing project's configuration.
///
/// Requires `UpdateProject` permission. Only provided fields are updated.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn update_project(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<UpdateProject>,
) -> Result<(StatusCode, Json<Project>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating project");

    auth_state
        .authorize_project(&mut conn, path_params.project_id, Permission::UpdateProject)
        .await?;

    let update_data = request.into_model();
    let project = conn
        .update_project(path_params.project_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Project updated");

    let project = Project::from_model(project);
    Ok((StatusCode::OK, Json(project)))
}

fn update_project_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update project")
        .description(
            "Updates an existing project's configuration. Only provided fields are updated.",
        )
        .response::<200, Json<Project>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Soft-deletes a project.
///
/// Requires `DeleteProject` permission. The project is marked as deleted
/// but data is retained for potential recovery.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        project_id = %path_params.project_id,
    )
)]
async fn delete_project(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting project");

    auth_state
        .authorize_project(&mut conn, path_params.project_id, Permission::DeleteProject)
        .await?;

    // Verify project exists before deletion
    if conn
        .find_project_by_id(path_params.project_id)
        .await?
        .is_none()
    {
        return Err(ErrorKind::NotFound
            .with_message(format!("Project not found: {}", path_params.project_id))
            .with_resource("project"));
    }

    conn.delete_project(path_params.project_id).await?;

    tracing::info!(target: TRACING_TARGET, "Project deleted");

    Ok(StatusCode::OK)
}

fn delete_project_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete project")
        .description("Soft-deletes a project. Data is retained for potential recovery.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all project-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/projects/",
            post_with(create_project, create_project_docs)
                .get_with(list_projects, list_projects_docs),
        )
        .api_route(
            "/projects/{project_id}/",
            get_with(read_project, read_project_docs)
                .patch_with(update_project, update_project_docs)
                .delete_with(delete_project, delete_project_docs),
        )
        .with_path_items(|item| item.tag("Projects"))
}
