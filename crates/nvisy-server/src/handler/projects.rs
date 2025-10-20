use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::models::{NewProject, NewProjectMember, Project, ProjectMember, UpdateProject};
use nvisy_postgres::queries::{ProjectMemberRepository, ProjectRepository};
use nvisy_postgres::types::ProjectRole;
use nvisy_postgres::{PgDatabase, PgError};
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
const TRACING_TARGET: &str = "nvisy::handler::projects";

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
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
struct CreateProjectRequest {
    #[validate(length(min = 3, max = 100))]
    pub display_name: String,
    #[validate(range(min = 60, max = 604800))]
    pub keep_for_sec: Option<u32>,
}

/// Response returned when a project is successfully created.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateProjectResponse {
    pub project_id: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl CreateProjectResponse {
    /// Creates a new [`CreateProjectResponse`].
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
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<CreateProjectRequest>,
) -> Result<(StatusCode, Json<CreateProjectResponse>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        display_name = %request.display_name,
        "creating new project",
    );

    let mut conn = pg_database.get_connection().await?;

    let response = conn
        .build_transaction()
        .run(|conn| {
            async move {
                let new_project = NewProject {
                    display_name: request.display_name,
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
    pub project_id: Uuid,

    pub display_name: String,
    pub member_role: ProjectRole,

    pub created_at: OffsetDateTime,
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
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ListProjectsResponse>)> {
    let mut conn = pg_database.get_connection().await?;

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
            project_items.push(ListProjectsResponseItem::from((project, membership)));
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
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<GetProjectResponse>)> {
    let mut conn = pg_database.get_connection().await?;

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
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectRequest {
    #[validate(length(min = 3, max = 100))]
    pub display_name: Option<String>,
}

/// Response for updated project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectResponse {
    pub project_id: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<Project> for UpdateProjectResponse {
    fn from(project: Project) -> Self {
        Self {
            project_id: project.id,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
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
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<UpdateProjectRequest>,
) -> Result<(StatusCode, Json<UpdateProjectResponse>)> {
    let mut conn = pg_database.get_connection().await?;

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
    pub project_id: Uuid,
    pub created_at: OffsetDateTime,
    pub deleted_at: OffsetDateTime,
}

impl From<Project> for DeleteProjectResponse {
    fn from(project: Project) -> Self {
        Self {
            project_id: project.id,
            created_at: project.created_at,
            deleted_at: project.deleted_at.unwrap_or_else(OffsetDateTime::now_utc),
        }
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
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
) -> Result<(StatusCode, Json<DeleteProjectResponse>)> {
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "project deletion requested - this is a destructive operation",
    );

    let mut conn = pg_database.get_connection().await?;

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
    use crate::handler::projects::routes;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let _server = create_test_server_with_router(|_| routes()).await?;

        // TODO: Add comprehensive integration tests for:
        // - Project creation with proper authorization
        // - Project listing with pagination
        // - Project updates with permission checks
        // - Project deletion with cascade handling
        // - Error scenarios and edge cases

        Ok(())
    }
}
