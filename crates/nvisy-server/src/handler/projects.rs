//! Project management handlers for CRUD operations.
//!
//! This module provides comprehensive project management functionality including
//! creating, reading, updating, and deleting projects. All operations are secured
//! with role-based access control.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewProject, NewProjectMember, UpdateProject};
use nvisy_postgres::query::{ProjectMemberRepository, ProjectRepository};
use nvisy_postgres::types::ProjectRole;
use nvisy_postgres::{PgClient, PgError};
use scoped_futures::ScopedFutureExt;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::authorize;
use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::request::project::{CreateProjectRequest, UpdateProjectRequest};
use crate::handler::response::project::{
    CreateProjectResponse, DeleteProjectResponse, GetProjectResponse, ListProjectsResponse,
    ListProjectsResponseItem, UpdateProjectResponse,
};
use crate::handler::{ErrorKind, ErrorResponse, PaginationRequest, Result};
use crate::service::ServiceState;

/// Tracing target for project operations.
const TRACING_TARGET: &str = "nvisy_server::handler::projects";

/// `Path` param for `{projectId}` handlers.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPathParams {
    /// Unique identifier of the project.
    pub project_id: Uuid,
}

/// Creates a new project.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/projects/", tag = "projects",
    request_body(
        content = CreateProjectRequest,
        description = "New project",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = CREATED,
            description = "Project created",
            body = CreateProjectResponse,
        ),
    ),
)]
async fn create_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<CreateProjectRequest>,
) -> Result<(StatusCode, Json<CreateProjectResponse>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        display_name = %request.display_name,
        "creating new project",
    );

    let mut conn = pg_client.get_connection().await?;

    let response = conn
        .build_transaction()
        .run(|conn| {
            async move {
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
                let project = ProjectRepository::create_project(conn, new_project).await?;

                let new_member = NewProjectMember {
                    project_id: project.id,
                    account_id: auth_claims.account_id,
                    member_role: ProjectRole::Owner,
                    ..Default::default()
                };
                ProjectMemberRepository::add_project_member(conn, new_member).await?;
                Ok::<CreateProjectResponse, PgError>(project.into())
            }
            .scope_boxed()
        })
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = response.project_id.to_string(),
        "new project created successfully",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Returns all projects for an account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/projects/", tag = "projects",
    request_body(
        content = PaginationRequest,
        description = "Pagination parameters",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Projects listed",
            body = ListProjectsResponse,
        ),
    ),
)]
async fn list_projects(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Json(pagination): Json<PaginationRequest>,
) -> Result<(StatusCode, Json<ListProjectsResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    // Use the combined query to fetch both project and membership data in a single query
    let project_memberships = ProjectMemberRepository::list_user_projects_with_details(
        &mut conn,
        auth_claims.account_id,
        pagination.into(),
    )
    .await?;

    // Convert to response items
    let projects: Vec<ListProjectsResponseItem> = project_memberships
        .into_iter()
        .map(|(project, membership)| (project, membership).into())
        .collect();

    let response = ListProjectsResponse::new(projects);

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_count = response.projects.len(),
        "listed user projects"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Gets a project by its project ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/projects/{projectId}/", tag = "projects",
    params(ProjectPathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Project details",
            body = GetProjectResponse,
        ),
    ),
)]
async fn read_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<GetProjectResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    authorize!(
        project: path_params.project_id,
        auth_claims, &mut conn,
        Permission::ViewDocuments,
    );

    let Some(project) =
        ProjectRepository::find_project_by_id(&mut conn, path_params.project_id).await?
    else {
        return Err(ErrorKind::NotFound.with_resource("project"));
    };

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "retrieved project details"
    );

    Ok((StatusCode::OK, Json(project.into())))
}

/// Updates a project by the project ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/projects/{projectId}/", tag = "projects",
    params(ProjectPathParams),
    request_body(
        content = UpdateProjectRequest,
        description = "Project changes",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Project changes",
            body = UpdateProjectResponse,
        ),
    ),
)]
async fn update_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<UpdateProjectRequest>,
) -> Result<(StatusCode, Json<UpdateProjectResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "updating project",
    );

    authorize!(
        project: path_params.project_id,
        auth_claims, &mut conn,
        Permission::UpdateProject,
    );

    let update_data = UpdateProject {
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

    let project =
        ProjectRepository::update_project(&mut conn, path_params.project_id, update_data).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "project updated successfully",
    );

    Ok((StatusCode::OK, Json(project.into())))
}

/// Deletes a project by its project ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    delete, path = "/projects/{projectId}/", tag = "projects",
    params(ProjectPathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Project deleted",
            body = DeleteProjectResponse,
        ),
    ),
)]
async fn delete_project(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<DeleteProjectResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Project deletion requested",
    );

    authorize!(
        project: path_params.project_id,
        auth_claims, &mut conn,
        Permission::DeleteProject,
    );

    ProjectRepository::delete_project(&mut conn, path_params.project_id).await?;
    let Some(deleted_project) =
        ProjectRepository::find_project_by_id(&mut conn, path_params.project_id).await?
    else {
        return Err(ErrorKind::NotFound.with_resource("project"));
    };

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "project deleted successfully",
    );

    Ok((StatusCode::OK, Json(deleted_project.into())))
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new()
        .routes(routes!(create_project, list_projects))
        .routes(routes!(read_project, update_project, delete_project))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn test_create_project_success() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = CreateProjectRequest {
            display_name: "Test Project".to_string(),
            description: Some("A test project".to_string()),
            keep_for_sec: Some(86400),
            auto_cleanup: Some(true),
            require_approval: Some(false),
            ..Default::default()
        };

        let response = server.post("/projects/").json(&request).await;
        response.assert_status(StatusCode::CREATED);

        let body: CreateProjectResponse = response.json();
        assert!(!body.project_id.is_nil());

        Ok(())
    }

    #[tokio::test]
    async fn test_create_project_invalid_name() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let request = CreateProjectRequest {
            display_name: "Ab".to_owned(),
            ..Default::default()
        };

        let response = server.post("/projects/").json(&request).await;
        response.assert_status_bad_request();

        Ok(())
    }

    #[tokio::test]
    async fn test_list_projects() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        // Create a project first
        let request = CreateProjectRequest {
            display_name: "List Test Project".to_string(),
            ..Default::default()
        };
        server.post("/projects/").json(&request).await;

        // List projects
        let pagination = PaginationRequest::default().with_limit(10);
        let response = server.get("/projects/").json(&pagination).await;
        response.assert_status_ok();

        let body: ListProjectsResponse = response.json();
        assert!(!body.projects.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_project() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        // Create a project
        let create_request = CreateProjectRequest {
            display_name: "Original Name".to_string(),
            ..Default::default()
        };
        let create_response = server.post("/projects/").json(&create_request).await;
        let created: CreateProjectResponse = create_response.json();

        // Update the project
        let update_request = UpdateProjectRequest {
            display_name: Some("Updated Name".to_string()),
            ..Default::default()
        };

        let response = server
            .patch(&format!("/projects/{}/", created.project_id))
            .json(&update_request)
            .await;
        response.assert_status_ok();

        Ok(())
    }

    #[tokio::test]
    async fn test_read_project() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        // Create a project
        let request = CreateProjectRequest {
            display_name: "Read Test".to_string(),
            description: Some("Test description".to_string()),
            ..Default::default()
        };
        let create_response = server.post("/projects/").json(&request).await;
        let created: CreateProjectResponse = create_response.json();

        // Read the project
        let response = server
            .get(&format!("/projects/{}/", created.project_id))
            .await;
        response.assert_status_ok();

        let body: GetProjectResponse = response.json();
        assert_eq!(body.project_id, created.project_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_project() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        // Create a project
        let request = CreateProjectRequest {
            display_name: "Delete Test".to_string(),
            ..Default::default()
        };
        let create_response = server.post("/projects/").json(&request).await;
        let created: CreateProjectResponse = create_response.json();

        // Delete the project
        let response = server
            .delete(&format!("/projects/{}/", created.project_id))
            .await;
        response.assert_status_ok();

        // Verify it's deleted by trying to read it
        let response = server
            .get(&format!("/projects/{}/", created.project_id))
            .await;
        response.assert_status_not_found();

        Ok(())
    }

    #[tokio::test]
    async fn test_read_nonexistent_project() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        let fake_id = Uuid::new_v4();
        let response = server.get(&format!("/projects/{}/", fake_id)).await;
        response.assert_status_not_found();

        Ok(())
    }
}
