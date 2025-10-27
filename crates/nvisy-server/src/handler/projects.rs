//! Project management handlers for CRUD operations.
//!
//! This module provides comprehensive project management functionality including
//! creating, reading, updating, and deleting projects. All operations are secured
//! with role-based access control.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::{NewProject, NewProjectMember, Project, ProjectMember, UpdateProject};
use nvisy_postgres::query::{ProjectMemberRepository, ProjectRepository};
use nvisy_postgres::types::ProjectRole;
use nvisy_postgres::{PgClient, PgError};
use scoped_futures::ScopedFutureExt;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use validator::Validate;

use crate::extract::auth::AuthProvider;
use crate::extract::{AuthState, Json, Path, ProjectPermission, ValidateJson};
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
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

/// Request payload for creating a new project.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "My Project",
    "description": "A project for document processing",
    "keepForSec": 86400,
    "autoCleanup": true,
    "requireApproval": false
}))]
struct CreateProjectRequest {
    /// Display name of the project.
    #[validate(length(min = 3, max = 100))]
    pub display_name: String,
    /// Description of the project.
    #[validate(length(min = 1, max = 200))]
    pub description: Option<String>,
    /// Duration in seconds to keep the original files.
    #[validate(range(min = 60, max = 604800))]
    pub keep_for_sec: Option<i32>,
    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: Option<bool>,
    /// Whether approval is required to processed files to be visible.
    pub require_approval: Option<bool>,
    /// Maximum number of members allowed in the project.
    #[validate(range(min = 1, max = 1000))]
    pub max_members: Option<i32>,
    /// Maximum storage size in megabytes allowed for the project.
    #[validate(range(min = 1, max = 1048576))]
    pub max_storage: Option<i32>,
    /// Whether comments are enabled for this project.
    pub enable_comments: Option<bool>,
}

/// Response returned when a project is successfully created.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateProjectResponse {
    /// ID of the created project.
    pub project_id: Uuid,
    /// Timestamp when the project was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the project was last updated.
    pub updated_at: OffsetDateTime,
}

impl CreateProjectResponse {
    /// Creates a new instance of [`CreateProjectResponse`].
    pub fn new(project: Project) -> Self {
        Self {
            project_id: project.id,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

impl From<Project> for CreateProjectResponse {
    #[inline]
    fn from(project: Project) -> Self {
        Self::new(project)
    }
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

/// Describes a project with role information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListProjectsResponseItem {
    /// ID of the project.
    pub project_id: Uuid,
    /// Display name of the project.
    pub display_name: String,
    /// Role of the member in the project.
    pub member_role: ProjectRole,
    /// Timestamp when the project was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the project was last updated.
    pub updated_at: OffsetDateTime,
}

impl ListProjectsResponseItem {
    /// Creates a new [`ListProjectsResponseItem`].
    pub fn new(project: Project, member: ProjectMember) -> Self {
        Self {
            project_id: project.id,
            display_name: project.display_name,
            member_role: member.member_role,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

impl From<(Project, ProjectMember)> for ListProjectsResponseItem {
    fn from((project, member): (Project, ProjectMember)) -> Self {
        Self::new(project, member)
    }
}

/// Response for listing all projects associated with the account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListProjectsResponse {
    /// List of projects associated with the account.
    pub projects: Vec<ListProjectsResponseItem>,
}

/// Returns all projects for an account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/projects/", tag = "projects",
    request_body(
        content = Pagination,
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
async fn list_all_projects(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ListProjectsResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    let memberships = ProjectMemberRepository::list_user_projects(
        &mut conn,
        auth_claims.account_id,
        pagination.into(),
    )
    .await?;

    // Convert memberships to response items
    let mut project_items = Vec::new();
    for membership in memberships {
        // Get project details for each membership
        if let Some(project) =
            ProjectRepository::find_project_by_id(&mut conn, membership.project_id).await?
        {
            project_items.push((project, membership).into());
        }
    }

    let response = ListProjectsResponse {
        projects: project_items,
    };

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_count = response.projects.len(),
        "listed user projects"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Response for getting a single project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct GetProjectResponse {
    /// ID of the project.
    pub project_id: Uuid,
}

impl From<Project> for GetProjectResponse {
    fn from(project: Project) -> Self {
        Self {
            project_id: project.id,
        }
    }
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

    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::ViewDocuments,
        )
        .await?;

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

/// Request payload to update project.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Updated Project Name",
    "description": "Updated description"
}))]
struct UpdateProjectRequest {
    /// Display name of the project.
    #[validate(length(min = 3, max = 100))]
    pub display_name: Option<String>,
    /// Description of the project.
    #[validate(length(min = 1, max = 200))]
    pub description: Option<String>,
    /// Duration in seconds to keep the original files.
    #[validate(range(min = 60, max = 604800))]
    pub keep_for_sec: Option<i32>,
    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: Option<bool>,
    /// Whether approval is required to processed files to be visible.
    pub require_approval: Option<bool>,
    /// Maximum number of members allowed in the project.
    #[validate(range(min = 1, max = 1000))]
    pub max_members: Option<i32>,
    /// Maximum storage size in megabytes allowed for the project.
    #[validate(range(min = 1, max = 1048576))]
    pub max_storage: Option<i32>,
    /// Whether comments are enabled for this project.
    pub enable_comments: Option<bool>,
}

/// Response for updated project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectResponse {
    /// ID of the project.
    pub project_id: Uuid,
    /// Timestamp when the project was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the project was last updated.
    pub updated_at: OffsetDateTime,
}

impl UpdateProjectResponse {
    /// Creates a new instance of `UpdateProjectResponse`.
    pub fn new(project: Project) -> Self {
        Self {
            project_id: project.id,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

impl From<Project> for UpdateProjectResponse {
    fn from(project: Project) -> Self {
        Self::new(project)
    }
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

    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::UpdateProject,
        )
        .await?;

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

/// Response returned after deleting an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DeleteProjectResponse {
    /// ID of the project.
    pub project_id: Uuid,
    /// Timestamp when the project was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the project was deleted.
    pub deleted_at: OffsetDateTime,
}

impl DeleteProjectResponse {
    /// Creates a new instance of [`DeleteProjectResponse`].
    pub fn new(project: Project) -> Self {
        Self {
            project_id: project.id,
            created_at: project.created_at,
            deleted_at: project.deleted_at.unwrap_or_else(OffsetDateTime::now_utc),
        }
    }
}

impl From<Project> for DeleteProjectResponse {
    fn from(project: Project) -> Self {
        Self::new(project)
    }
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
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "project deletion requested - this is a destructive operation",
    );

    let mut conn = pg_client.get_connection().await?;

    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::DeleteProject,
        )
        .await?;

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
        .routes(routes!(create_project, list_all_projects))
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
        let pagination = Pagination::default().with_limit(10);
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
