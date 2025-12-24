//! Project invite response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{InviteStatus, ProjectRole};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use uuid::Uuid;

// TODO: Add invitee emails

/// Project invite with complete information.
///
/// This response includes all the essential information about an
/// invitation, including the unique invite ID that can be used to track or cancel
/// the invitation later.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    /// Unique identifier of the invitation.
    pub invite_id: Uuid,
    /// ID of the project the invitation is for.
    pub project_id: Uuid,
    /// Account ID if the invitee has an account.
    pub invitee_id: Option<Uuid>,
    /// Role the invitee will have if they accept.
    pub invited_role: ProjectRole,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation expires.
    pub expires_at: Timestamp,
    /// When the invitation was created.
    pub created_at: Timestamp,
    /// When the invitation was last updated.
    pub updated_at: Timestamp,
}

impl From<model::ProjectInvite> for Invite {
    fn from(invite: model::ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            project_id: invite.project_id,
            invitee_id: invite.invitee_id,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at.into(),
            created_at: invite.created_at.into(),
            updated_at: invite.updated_at.into(),
        }
    }
}

/// Response for listing project invitations.
pub type Invites = Vec<Invite>;
