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
