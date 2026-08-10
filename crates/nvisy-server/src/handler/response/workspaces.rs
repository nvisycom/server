//! Workspace response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{Handle, NotificationEvent, WorkspaceRole, WorkspaceSettings};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AccountRef, Page};

/// Workspace response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    /// URL-safe workspace identifier.
    pub slug: Handle,
    /// Display name of the workspace.
    pub display_name: String,
    /// Description of the workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Serve path of the workspace's avatar (logo), when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    /// Tags associated with the workspace.
    pub tags: Vec<String>,
    /// Workspace settings (approval requirement, data-retention rules).
    pub settings: WorkspaceSettings,
    /// Account that created this workspace.
    pub created_by: AccountRef,
    /// Role of the member in the workspace.
    pub member_role: WorkspaceRole,
    /// Timestamp when the workspace was created.
    pub created_at: Timestamp,
    /// Timestamp when the workspace was last updated.
    pub updated_at: Timestamp,
}

impl Workspace {
    /// Creates a new instance of [`Workspace`] as an owner.
    pub fn from_model(workspace: model::Workspace, created_by: AccountRef) -> Self {
        let tags = workspace.get_tags();
        let settings = WorkspaceSettings::from_value(&workspace.settings);
        Self {
            slug: workspace.slug,
            display_name: workspace.display_name,
            description: workspace.description,
            avatar_url: workspace.avatar_url,
            tags,
            settings,
            created_by,
            member_role: WorkspaceRole::Owner,
            created_at: workspace.created_at.into(),
            updated_at: workspace.updated_at.into(),
        }
    }

    /// Creates a new instance of [`Workspace`] with role information.
    pub fn from_model_with_membership(
        workspace: model::Workspace,
        member: model::WorkspaceMember,
        created_by: AccountRef,
    ) -> Self {
        let tags = workspace.get_tags();
        let settings = WorkspaceSettings::from_value(&workspace.settings);
        Self {
            slug: workspace.slug,
            display_name: workspace.display_name,
            description: workspace.description,
            avatar_url: workspace.avatar_url,
            tags,
            settings,
            created_by,
            member_role: member.member_role,
            created_at: workspace.created_at.into(),
            updated_at: workspace.updated_at.into(),
        }
    }
}

/// Paginated list of workspaces.
pub type WorkspacesPage = Page<Workspace>;

/// Response for notification settings within a workspace.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
