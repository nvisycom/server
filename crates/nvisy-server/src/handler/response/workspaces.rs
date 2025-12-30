//! Workspace response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::WorkspaceRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Workspace response.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    /// ID of the workspace.
    pub workspace_id: Uuid,
    /// Display name of the workspace.
    pub display_name: String,
    /// Description of the workspace.
    pub description: Option<String>,
    /// Duration in seconds to keep the original files (optional).
    pub keep_for_sec: Option<i32>,
    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: bool,
    /// Whether approval is required to processed files to be visible.
    pub require_approval: bool,
    /// Maximum number of members allowed in the workspace.
    pub max_members: Option<i32>,
    /// Maximum storage size in megabytes allowed for the workspace.
    pub max_storage: Option<i32>,
    /// Whether comments are enabled for this workspace.
    pub enable_comments: bool,
    /// Role of the member in the workspace.
    pub member_role: WorkspaceRole,
    /// Timestamp when the workspace was created.
    pub created_at: Timestamp,
    /// Timestamp when the workspace was last updated.
    pub updated_at: Timestamp,
}

impl Workspace {
    /// Creates a new instance of [`Workspace`] as an owner.
    pub fn from_model(workspace: model::Workspace) -> Self {
        Self {
            workspace_id: workspace.id,
            display_name: workspace.display_name,
            description: workspace.description,
            keep_for_sec: workspace.keep_for_sec,
            auto_cleanup: workspace.auto_cleanup,
            require_approval: workspace.require_approval,
            max_members: workspace.max_members,
            max_storage: workspace.max_storage,
            enable_comments: workspace.enable_comments,
            member_role: WorkspaceRole::Owner,
            created_at: workspace.created_at.into(),
            updated_at: workspace.updated_at.into(),
        }
    }

    /// Creates a new instance of [`Workspace`] with role information.
    pub fn from_model_with_membership(
        workspace: model::Workspace,
        member: model::WorkspaceMember,
    ) -> Self {
        Self {
            workspace_id: workspace.id,
            display_name: workspace.display_name,
            description: workspace.description,
            keep_for_sec: workspace.keep_for_sec,
            auto_cleanup: workspace.auto_cleanup,
            require_approval: workspace.require_approval,
            max_members: workspace.max_members,
            max_storage: workspace.max_storage,
            enable_comments: workspace.enable_comments,
            member_role: member.member_role,
            created_at: workspace.created_at.into(),
            updated_at: workspace.updated_at.into(),
        }
    }
}

/// Response for listing all workspaces associated with the account.
pub type Workspaces = Vec<Workspace>;
