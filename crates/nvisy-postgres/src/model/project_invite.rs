//! Project invite model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::project_invites;
use crate::types::{InviteStatus, ProjectRole};

/// Project invitation model representing an invitation to join a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectInvite {
    /// Unique invitation identifier
    pub id: Uuid,
    /// Reference to the project
    pub project_id: Uuid,
    /// Email address of the invitee
    pub invitee_email: String,
    /// Account ID if invitee is already registered
    pub invitee_id: Option<Uuid>,
    /// Role to be assigned upon acceptance
    pub invited_role: ProjectRole,
    /// Optional message from the inviter
    pub invite_message: String,
    /// Unique token for accepting the invitation
    pub invite_token: String,
    /// Current status of the invitation
    pub invite_status: InviteStatus,
    /// When the invitation expires
    pub expires_at: OffsetDateTime,
    /// Account that created the invitation
    pub created_by: Uuid,
    /// Account that last updated the invitation
    pub updated_by: Uuid,
    /// Timestamp when invitee responded
    pub responded_at: Option<OffsetDateTime>,
    /// Timestamp when invitation was created
    pub created_at: OffsetDateTime,
    /// Timestamp when invitation was last updated
    pub updated_at: OffsetDateTime,
}

/// Data for creating a new project invitation.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = project_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectInvite {
    /// Project ID
    pub project_id: Uuid,
    /// Invitee email
    pub invitee_email: String,
    /// Invitee ID
    pub invitee_id: Option<Uuid>,
    /// Invited role
    pub invited_role: ProjectRole,
    /// Invite message
    pub invite_message: String,
    /// Invite token
    pub invite_token: String,
    /// Expires at
    pub expires_at: OffsetDateTime,
    /// Created by
    pub created_by: Uuid,
    /// Updated by
    pub updated_by: Uuid,
}

/// Data for updating a project invitation.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectInvite {
    /// Invitee ID
    pub invitee_id: Option<Uuid>,
    /// Invite status
    pub invite_status: Option<InviteStatus>,
    /// Updated by
    pub updated_by: Option<Uuid>,
    /// Responded at
    pub responded_at: Option<OffsetDateTime>,
}

impl Default for NewProjectInvite {
    fn default() -> Self {
        Self {
            project_id: Uuid::new_v4(),
            invitee_email: String::new(),
            invitee_id: None,
            invited_role: ProjectRole::Viewer,
            invite_message: String::new(),
            invite_token: String::new(),
            expires_at: OffsetDateTime::now_utc() + time::Duration::days(7),
            created_by: Uuid::new_v4(),
            updated_by: Uuid::new_v4(),
        }
    }
}

impl ProjectInvite {
    /// Returns whether the invitation is still valid.
    pub fn is_valid(&self) -> bool {
        self.invite_status == InviteStatus::Pending && self.expires_at > OffsetDateTime::now_utc()
    }

    /// Returns whether the invitation has expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at <= OffsetDateTime::now_utc()
    }

    /// Returns whether the invitation can still be used.
    pub fn can_be_used(&self) -> bool {
        self.is_valid() && !self.is_expired()
    }

    /// Returns whether the invitation is pending.
    pub fn is_pending(&self) -> bool {
        self.invite_status == InviteStatus::Pending
    }

    /// Returns whether the invitation was accepted.
    pub fn is_accepted(&self) -> bool {
        self.invite_status == InviteStatus::Accepted
    }

    /// Returns whether the invitation was declined.
    pub fn is_declined(&self) -> bool {
        self.invite_status == InviteStatus::Declined
    }

    /// Returns whether the invitation was canceled.
    pub fn is_canceled(&self) -> bool {
        self.invite_status == InviteStatus::Canceled
    }

    /// Returns whether the invitation was revoked.
    pub fn is_revoked(&self) -> bool {
        self.invite_status == InviteStatus::Revoked
    }

    /// Returns whether the invitation can be resent (was declined, expired, or revoked).
    pub fn can_be_resent(&self) -> bool {
        self.invite_status.can_be_resent()
    }

    /// Returns whether the invitation is in a final state.
    pub fn is_resolved(&self) -> bool {
        self.invite_status.is_resolved() || self.invite_status.is_terminated()
    }
}
