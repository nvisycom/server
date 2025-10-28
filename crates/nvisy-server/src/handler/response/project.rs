//! Project response types.

use nvisy_postgres::model::{Project, ProjectMember};
use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Response returned when a project is successfully created.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectResponse {
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

/// Describes a project with role information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectsResponseItem {
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
pub struct ListProjectsResponse {
    /// List of projects associated with the account.
    pub projects: Vec<ListProjectsResponseItem>,
}

impl ListProjectsResponse {
    /// Creates a new instance of [`ListProjectsResponse`].
    pub fn new(projects: Vec<ListProjectsResponseItem>) -> Self {
        Self { projects }
    }
}

/// Response for getting a single project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetProjectResponse {
    /// ID of the project.
    pub project_id: Uuid,
}

impl GetProjectResponse {
    /// Creates a new instance of [`GetProjectResponse`].
    pub fn new(project: Project) -> Self {
        Self {
            project_id: project.id,
        }
    }
}

impl From<Project> for GetProjectResponse {
    fn from(project: Project) -> Self {
        Self::new(project)
    }
}

/// Response for updated project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectResponse {
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

/// Response returned after deleting an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProjectResponse {
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
