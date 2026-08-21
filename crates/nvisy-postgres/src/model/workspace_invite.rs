//! Workspace invite model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_invites;
use crate::types::{HasCreatedAt, HasUpdatedAt, InviteStatus, WorkspaceRole};

/// Workspace invitation model representing an invitation to join a workspace.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceInvite {
    /// Unique invitation identifier.
    pub id: Uuid,
    /// Reference to the workspace.
    pub workspace_id: Uuid,
    /// Email address of the invitee (null for open invite codes).
    pub invitee_email: Option<String>,
    /// Role to be assigned upon acceptance.
    pub invited_role: WorkspaceRole,
    /// Unique token for accepting the invitation.
    pub invite_token: String,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation expires.
    pub expires_at: Timestamp,
    /// Account that created the invitation.
    pub created_by: Uuid,
    /// Account that last updated the invitation.
    pub updated_by: Uuid,
    /// Timestamp when invitee responded.
    pub responded_at: Option<Timestamp>,
    /// Timestamp when invitation was created.
    pub created_at: Timestamp,
    /// Timestamp when invitation was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new workspace invitation.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceInvite {
    /// Workspace ID.
    pub workspace_id: Uuid,
    /// Email address of the invitee (null for open invite codes).
    pub invitee_email: Option<String>,
    /// Invited role.
    pub invited_role: Option<WorkspaceRole>,
    /// Invite token.
    pub invite_token: Option<String>,
    /// Expires at.
    pub expires_at: Option<Timestamp>,
    /// Created by.
    pub created_by: Uuid,
    /// Updated by.
    pub updated_by: Uuid,
}

/// Data for updating a workspace invitation.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceInvite {
    /// Invite status.
    pub invite_status: Option<InviteStatus>,
    /// Responded at.
    pub responded_at: Option<Option<Timestamp>>,
    /// Updated by.
    pub updated_by: Option<Uuid>,
}

impl WorkspaceInvite {
    /// Returns whether the invitation is still valid.
    pub fn is_valid(&self) -> bool {
        self.invite_status == InviteStatus::Pending
            && jiff::Timestamp::from(self.expires_at) > jiff::Timestamp::now()
    }

    /// Returns whether the invitation has expired.
    pub fn is_expired(&self) -> bool {
        jiff::Timestamp::from(self.expires_at) <= jiff::Timestamp::now()
    }

    /// Returns whether the invitation can still be used.
    pub fn can_be_used(&self) -> bool {
        self.is_valid() && !self.is_expired()
    }
}

impl HasCreatedAt for WorkspaceInvite {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceInvite {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}
