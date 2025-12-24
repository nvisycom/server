//! Project management handlers for CRUD operations.
//!
//! This module provides comprehensive project management functionality including
//! creating, reading, updating, and deleting projects. All operations are secured
//! with role-based access control.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{self, NewProject, NewProjectMember};
use nvisy_postgres::query::{ProjectMemberRepository, ProjectRepository};
use nvisy_postgres::types::ProjectRole;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::request::{CreateProject, Pagination, ProjectPathParams, UpdateProject};
use crate::handler::response::{Project, Projects};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project operations.
const TRACING_TARGET: &str = "nvisy_server::handler::projects";

/// Creates a new project.
#[tracing::instrument(skip_all)]
async fn create_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<CreateProject>,
) -> Result<(StatusCode, Json<Project>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        display_name = %request.display_name,
        "Creating new project",
    );

    let new_project = NewProject {
        display_name: request.display_name,
        description: request.description,
        keep_for_sec: request.keep_for_sec,
        auto_cleanup: request.auto_cleanup,
        require_approval: request.require_approval,
        max_members: request.max_members,
        max_storage: request.max_storage,
        enable_comments: request.enable_comments,
        created_by: auth_claims.account_id,
        ..Default::default()
    };
    let project = pg_client.create_project(new_project).await?;

    let new_member = NewProjectMember {
        project_id: project.id,
        account_id: auth_claims.account_id,
        member_role: ProjectRole::Owner,
        ..Default::default()
    };
    pg_client.add_project_member(new_member).await?;

    let response = Project::from_model(project);

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = response.project_id.to_string(),
        "New project created successfully",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Returns all projects for an account.
#[tracing::instrument(skip_all)]
async fn list_projects(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Projects>)> {
    let project_memberships = pg_client
        .list_user_projects_with_details(auth_claims.account_id, pagination.into())
        .await?;

    // Convert to response items
    let projects: Projects = project_memberships
        .into_iter()
        .map(|(project, membership)| Project::from_model_with_membership(project, membership))
        .collect();

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_count = projects.len(),
        "Listed user projects"
    );

    Ok((StatusCode::OK, Json(projects)))
}

/// Gets a project by its project ID.
#[tracing::instrument(skip_all)]
async fn read_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<Project>)> {
    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::ViewProject)
        .await?;

    let Some(project) = pg_client.find_project_by_id(path_params.project_id).await? else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Project not found: {}", path_params.project_id))
            .with_resource("project"));
    };

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Retrieved project details"
    );

    let project = Project::from_model(project);
    Ok((StatusCode::OK, Json(project)))
}

/// Updates a project by the project ID.
#[tracing::instrument(skip_all)]
async fn update_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<UpdateProject>,
) -> Result<(StatusCode, Json<Project>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Updating project",
    );

    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::UpdateProject,
        )
        .await?;

    let update_data = model::UpdateProject {
        display_name: request.display_name,
        description: request.description,
        keep_for_sec: request.keep_for_sec,
        auto_cleanup: request.auto_cleanup,
        require_approval: request.require_approval,
        max_members: request.max_members,
        max_storage: request.max_storage,
        enable_comments: request.enable_comments,
        ..Default::default()
    };

    let project = pg_client
        .update_project(path_params.project_id, update_data)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Project updated successfully",
    );

    let project = Project::from_model(project);
    Ok((StatusCode::OK, Json(project)))
}

/// Deletes a project by its project ID.
#[tracing::instrument(skip_all)]
async fn delete_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Project deletion requested",
    );

    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::DeleteProject,
        )
        .await?;

    // Verify project exists before deletion
    let Some(_project) = pg_client.find_project_by_id(path_params.project_id).await? else {
        return Err(ErrorKind::NotFound
            .with_message(format!("Project not found: {}", path_params.project_id))
            .with_resource("project"));
    };

    pg_client.delete_project(path_params.project_id).await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Project deleted successfully",
    );

    Ok(StatusCode::OK)
}

/// Returns a [`Router`] with all related routes.
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

