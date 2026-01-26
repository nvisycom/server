//! Workspace invite model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_invites;
use crate::types::{HasCreatedAt, HasUpdatedAt, InviteStatus, RECENTLY_SENT_HOURS, WorkspaceRole};

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

    /// Returns whether the invitee has responded to the invitation.
    pub fn has_response(&self) -> bool {
        self.responded_at.is_some()
    }

    /// Returns whether the invitation was sent recently.
    pub fn is_recently_sent(&self) -> bool {
        self.was_created_within(jiff::Span::new().hours(RECENTLY_SENT_HOURS))
    }

    /// Returns the time remaining until expiration.
    pub fn time_until_expiry(&self) -> Option<jiff::Span> {
        let now = jiff::Timestamp::now();
        let expires_at = jiff::Timestamp::from(self.expires_at);
        if expires_at > now {
            Some(expires_at - now)
        } else {
            None
        }
    }

    /// Returns whether the invitation is expiring soon (within 24 hours).
    pub fn is_expiring_soon(&self) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining.total(jiff::Unit::Second).ok()
                <= jiff::Span::new().days(1).total(jiff::Unit::Second).ok()
        } else {
            false
        }
    }

    /// Returns the age of the invitation since creation.
    pub fn age(&self) -> jiff::Span {
        jiff::Timestamp::now() - jiff::Timestamp::from(self.created_at)
    }

    /// Returns the response time if the invitation was responded to.
    pub fn response_time(&self) -> Option<jiff::Span> {
        self.responded_at.map(|responded_at| {
            jiff::Timestamp::from(responded_at) - jiff::Timestamp::from(self.created_at)
        })
    }

    /// Returns whether the invitation is for a specific email.
    pub fn is_for_specific_email(&self) -> bool {
        self.invitee_email.is_some()
    }

    /// Returns whether this is an open invitation (no specific email).
    pub fn is_open_invitation(&self) -> bool {
        self.invitee_email.is_none()
    }

    /// Returns whether the invitation grants owner privileges.
    pub fn grants_owner_access(&self) -> bool {
        matches!(self.invited_role, WorkspaceRole::Owner)
    }

    /// Returns whether the invitation can be canceled.
    pub fn can_be_canceled(&self) -> bool {
        self.is_pending() && !self.is_expired()
    }

    /// Returns whether the invitation can be resent.
    pub fn can_be_resent(&self) -> bool {
        self.is_expired() || self.is_declined()
    }

    /// Returns the invitation token (shortened for display).
    pub fn token_short(&self) -> String {
        if self.invite_token.len() > 8 {
            format!("{}...", &self.invite_token[..8])
        } else {
            self.invite_token.clone()
        }
    }

    /// Returns whether the invitation needs immediate attention.
    pub fn needs_attention(&self) -> bool {
        self.is_expiring_soon() || self.is_expired()
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
