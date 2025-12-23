//! Project response types.

use nvisy_postgres::model;
use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Project response.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// ID of the project.
    pub project_id: Uuid,
    /// Display name of the project.
    pub display_name: String,
    /// Description of the project.
    pub description: Option<String>,
    /// Duration in seconds to keep the original files (optional).
    pub keep_for_sec: Option<i32>,
    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: bool,
    /// Whether approval is required to processed files to be visible.
    pub require_approval: bool,
    /// Maximum number of members allowed in the project.
    pub max_members: Option<i32>,
    /// Maximum storage size in megabytes allowed for the project.
    pub max_storage: Option<i32>,
    /// Whether comments are enabled for this project.
    pub enable_comments: bool,
    /// Role of the member in the project.
    pub member_role: ProjectRole,
    /// Timestamp when the project was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the project was last updated.
    pub updated_at: OffsetDateTime,
}

impl Project {
    /// Creates a new instance of [`Project`] as an owner.
    pub fn from_model(project: model::Project) -> Self {
        Self {
            project_id: project.id,
            display_name: project.display_name,
            description: project.description,
            keep_for_sec: project.keep_for_sec,
            auto_cleanup: project.auto_cleanup,
            require_approval: project.require_approval,
            max_members: project.max_members,
            max_storage: project.max_storage,
            enable_comments: project.enable_comments,
            member_role: ProjectRole::Owner,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }

    /// Creates a new instance of [`Project`] with role information.
    pub fn from_model_with_membership(
        project: model::Project,
        member: model::ProjectMember,
    ) -> Self {
        Self {
            project_id: project.id,
            display_name: project.display_name,
            description: project.description,
            keep_for_sec: project.keep_for_sec,
            auto_cleanup: project.auto_cleanup,
            require_approval: project.require_approval,
            max_members: project.max_members,
            max_storage: project.max_storage,
            enable_comments: project.enable_comments,
            member_role: member.member_role,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

/// Response for listing all projects associated with the account.
pub type Projects = Vec<Project>;
