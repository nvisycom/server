//! Workspace response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{NotificationEvent, WorkspaceRole};
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
    /// Tags associated with the workspace.
    pub tags: Vec<String>,
    /// Whether to automatically delete processed files after expiration.
    pub auto_cleanup: bool,
    /// Whether approval is required to processed files to be visible.
    pub require_approval: bool,
    /// Whether comments are enabled for this workspace.
    pub enable_comments: bool,
    /// ID of the account that created the workspace.
    pub created_by: Uuid,
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
        let tags = workspace.get_tags();
        Self {
            workspace_id: workspace.id,
            display_name: workspace.display_name,
            description: workspace.description,
            tags,
            auto_cleanup: workspace.auto_cleanup,
            require_approval: workspace.require_approval,
            enable_comments: workspace.enable_comments,
            created_by: workspace.created_by,
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
        let tags = workspace.get_tags();
        Self {
            workspace_id: workspace.id,
            display_name: workspace.display_name,
            description: workspace.description,
            tags,
            auto_cleanup: workspace.auto_cleanup,
            require_approval: workspace.require_approval,
            enable_comments: workspace.enable_comments,
            created_by: workspace.created_by,
            member_role: member.member_role,
            created_at: workspace.created_at.into(),
            updated_at: workspace.updated_at.into(),
        }
    }
}

/// Response for listing all workspaces associated with the account.
pub type Workspaces = Vec<Workspace>;

/// Response for notification settings within a workspace.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    /// Whether to send email notifications.
    pub notify_via_email: bool,
    /// Notification events to receive in-app.
    pub notification_events_app: Vec<NotificationEvent>,
    /// Notification events to receive via email.
    pub notification_events_email: Vec<NotificationEvent>,
}

impl NotificationSettings {
    /// Creates a new instance from a workspace member model.
    pub fn from_member(member: &model::WorkspaceMember) -> Self {
        Self {
            notify_via_email: member.notify_via_email,
            notification_events_app: member.app_notification_events(),
            notification_events_email: member.email_notification_events(),
        }
    }
}
