//! Workspace member model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_members;
use crate::types::{HasCreatedAt, HasOwnership, HasUpdatedAt, NotificationEvent, WorkspaceRole};

/// Workspace member model representing a user's membership in a workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceMember {
    /// Reference to the workspace.
    pub workspace_id: Uuid,
    /// Reference to the member's account.
    pub account_id: Uuid,
    /// Member's role in the workspace.
    pub member_role: WorkspaceRole,
    /// Whether to send email notifications.
    pub notify_via_email: bool,
    /// Notification events to receive in-app.
    pub notification_events_app: Vec<Option<NotificationEvent>>,
    /// Notification events to receive via email.
    pub notification_events_email: Vec<Option<NotificationEvent>>,
    /// Account that created this membership.
    pub created_by: Uuid,
    /// Account that last updated this membership.
    pub updated_by: Uuid,
    /// Timestamp when membership was created.
    pub created_at: Timestamp,
    /// Timestamp when membership was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new workspace member.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceMember {
    /// Workspace ID.
    pub workspace_id: Uuid,
    /// Account ID.
    pub account_id: Uuid,
    /// Member role.
    pub member_role: WorkspaceRole,
    /// Whether to send email notifications.
    pub notify_via_email: bool,
    /// Notification events to receive in-app.
    pub notification_events_app: Vec<Option<NotificationEvent>>,
    /// Notification events to receive via email.
    pub notification_events_email: Vec<Option<NotificationEvent>>,
    /// Created by.
    pub created_by: Uuid,
    /// Updated by.
    pub updated_by: Uuid,
}

impl NewWorkspaceMember {
    /// Creates a new workspace membership with the specified role.
    pub fn new(workspace_id: Uuid, account_id: Uuid, role: WorkspaceRole) -> Self {
        Self {
            workspace_id,
            account_id,
            member_role: role,
            created_by: account_id,
            updated_by: account_id,
            ..Default::default()
        }
    }

    /// Creates a new owner membership for a workspace.
    pub fn new_owner(workspace_id: Uuid, account_id: Uuid) -> Self {
        Self::new(workspace_id, account_id, WorkspaceRole::Owner)
    }
}

/// Data for updating a workspace member.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceMember {
    /// Member role.
    pub member_role: Option<WorkspaceRole>,
    /// Whether to send email notifications.
    pub notify_via_email: Option<bool>,
    /// Notification events to receive in-app.
    pub notification_events_app: Option<Vec<Option<NotificationEvent>>>,
    /// Notification events to receive via email.
    pub notification_events_email: Option<Vec<Option<NotificationEvent>>>,
    /// Updated by.
    pub updated_by: Option<Uuid>,
}

impl WorkspaceMember {
    /// Returns whether the member has owner privileges.
    pub fn is_owner(&self) -> bool {
        matches!(self.member_role, WorkspaceRole::Owner)
    }

    /// Returns whether the member can invite others.
    pub fn can_invite(&self) -> bool {
        matches!(self.member_role, WorkspaceRole::Owner)
    }

    /// Returns whether the member is a regular member.
    pub fn is_member(&self) -> bool {
        self.member_role == WorkspaceRole::Member
    }

    /// Returns whether the member is a guest.
    pub fn is_guest(&self) -> bool {
        self.member_role == WorkspaceRole::Guest
    }

    /// Returns whether the member can edit workspace content.
    pub fn can_edit(&self) -> bool {
        matches!(
            self.member_role,
            WorkspaceRole::Owner | WorkspaceRole::Member
        )
    }

    /// Returns whether the member can manage other members.
    pub fn can_manage_members(&self) -> bool {
        matches!(self.member_role, WorkspaceRole::Owner)
    }

    /// Returns the in-app notification events (without None values).
    pub fn app_notification_events(&self) -> Vec<NotificationEvent> {
        self.notification_events_app
            .iter()
            .filter_map(|e| *e)
            .collect()
    }

    /// Returns the email notification events (without None values).
    pub fn email_notification_events(&self) -> Vec<NotificationEvent> {
        self.notification_events_email
            .iter()
            .filter_map(|e| *e)
            .collect()
    }

    /// Returns whether the member has any in-app notification events enabled.
    pub fn has_app_notifications(&self) -> bool {
        self.notification_events_app.iter().any(|e| e.is_some())
    }

    /// Returns whether the member has any email notification events enabled.
    pub fn has_email_notifications(&self) -> bool {
        self.notify_via_email && self.notification_events_email.iter().any(|e| e.is_some())
    }

    /// Returns whether the member can perform administrative actions.
    pub fn can_administrate(&self) -> bool {
        self.is_owner()
    }

    /// Returns whether the member can modify workspace settings.
    pub fn can_modify_settings(&self) -> bool {
        self.is_owner()
    }

    /// Returns whether the member can delete the workspace.
    pub fn can_delete_workspace(&self) -> bool {
        self.is_owner()
    }

    /// Returns whether the member can be promoted to the given role.
    pub fn can_be_promoted_to(&self, role: WorkspaceRole) -> bool {
        role > self.member_role
    }

    /// Returns whether the member can be demoted to the given role.
    pub fn can_be_demoted_to(&self, role: WorkspaceRole) -> bool {
        role < self.member_role
    }
}

impl HasCreatedAt for WorkspaceMember {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceMember {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasOwnership for WorkspaceMember {
    fn created_by(&self) -> Uuid {
        self.created_by
    }

    fn updated_by(&self) -> Option<Uuid> {
        Some(self.updated_by)
    }
}
