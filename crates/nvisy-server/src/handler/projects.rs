//! Project management handlers for CRUD operations.
//!
//! This module provides comprehensive project management functionality including
//! creating, reading, updating, and deleting projects. All operations are secured
//! with role-based access control.

use aide::axum::ApiRouter;
use axum::http::StatusCode;
use nvisy_postgres::PgError;
use nvisy_postgres::model::{NewProjectMember, Project as ProjectModel, ProjectMember};
use nvisy_postgres::query::{ProjectMemberRepository, ProjectRepository};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{CreateProject, Pagination, ProjectPathParams, UpdateProject};
use crate::handler::response::{Project, Projects};
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
    tracing::info!(target: TRACING_TARGET, "Creating new project");

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
        "Project created successfully",
    );

    Ok((StatusCode::CREATED, Json(response)))
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

    tracing::debug!(
        target: TRACING_TARGET,
        project_count = projects.len(),
        "Listed user projects",
    );

    Ok((StatusCode::OK, Json(projects)))
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

    tracing::debug!(target: TRACING_TARGET, "Retrieved project details");

    let project = Project::from_model(project);
    Ok((StatusCode::OK, Json(project)))
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
    tracing::info!(target: TRACING_TARGET, "Updating project");

    auth_state
        .authorize_project(&mut conn, path_params.project_id, Permission::UpdateProject)
        .await?;

    let update_data = request.into_model();
    let project = conn
        .update_project(path_params.project_id, update_data)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Project updated successfully");

    let project = Project::from_model(project);
    Ok((StatusCode::OK, Json(project)))
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
    tracing::warn!(target: TRACING_TARGET, "Project deletion requested");

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

    tracing::warn!(target: TRACING_TARGET, "Project deleted successfully");

    Ok(StatusCode::OK)
}

/// Returns a [`Router`] with all project-related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/projects/", post(create_project))
        .api_route("/projects/", get(list_projects))
        .api_route("/projects/:project_id/", get(read_project))
        .api_route("/projects/:project_id/", patch(update_project))
        .api_route("/projects/:project_id/", delete(delete_project))
}
