//! Project invite model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::project_invites;
use crate::types::constants::invite;
use crate::types::{HasCreatedAt, HasUpdatedAt, InviteStatus, ProjectRole};

/// Project invitation model representing an invitation to join a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectInvite {
    /// Unique invitation identifier.
    pub id: Uuid,
    /// Reference to the project.
    pub project_id: Uuid,
    /// Account ID if invitee is already registered.
    pub invitee_id: Option<Uuid>,
    /// Role to be assigned upon acceptance.
    pub invited_role: ProjectRole,
    /// Optional message from the inviter.
    pub invite_message: String,
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

/// Data for creating a new project invitation.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = project_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectInvite {
    /// Project ID.
    pub project_id: Uuid,
    /// Invitee ID.
    pub invitee_id: Option<Uuid>,
    /// Invited role.
    pub invited_role: Option<ProjectRole>,
    /// Invite message.
    pub invite_message: Option<String>,
    /// Invite token.
    pub invite_token: Option<String>,
    /// Expires at.
    pub expires_at: Option<Timestamp>,
    /// Created by.
    pub created_by: Uuid,
    /// Updated by.
    pub updated_by: Uuid,
}

/// Data for updating a project invitation.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectInvite {
    /// Invite status.
    pub invite_status: Option<InviteStatus>,
    /// Responded at.
    pub responded_at: Option<Timestamp>,
    /// Updated by.
    pub updated_by: Option<Uuid>,
}

impl ProjectInvite {
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
        self.was_created_within(jiff::Span::new().hours(invite::RECENTLY_SENT_HOURS))
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

    /// Returns whether the invitation has a custom message.
    pub fn has_message(&self) -> bool {
        !self.invite_message.is_empty()
    }

    /// Returns whether the invitation is for a specific user.
    pub fn is_for_specific_user(&self) -> bool {
        self.invitee_id.is_some()
    }

    /// Returns whether this is an open invitation (no specific user).
    pub fn is_open_invitation(&self) -> bool {
        self.invitee_id.is_none()
    }

    /// Returns whether the invitation grants admin privileges.
    pub fn grants_admin_access(&self) -> bool {
        matches!(self.invited_role, ProjectRole::Admin)
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

impl HasCreatedAt for ProjectInvite {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for ProjectInvite {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}
