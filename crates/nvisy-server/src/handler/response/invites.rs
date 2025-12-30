//! Workspace invite response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{InviteStatus, WorkspaceRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Workspace invite with complete information.
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
    /// ID of the workspace the invitation is for.
    pub workspace_id: Uuid,
    /// Account ID if the invitee has an account.
    pub invitee_id: Option<Uuid>,
    /// Role the invitee will have if they accept.
    pub invited_role: WorkspaceRole,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation expires.
    pub expires_at: Timestamp,
    /// When the invitation was created.
    pub created_at: Timestamp,
    /// When the invitation was last updated.
    pub updated_at: Timestamp,
}

impl From<model::WorkspaceInvite> for Invite {
    fn from(invite: model::WorkspaceInvite) -> Self {
        Self {
            invite_id: invite.id,
            workspace_id: invite.workspace_id,
            invitee_id: invite.invitee_id,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at.into(),
            created_at: invite.created_at.into(),
            updated_at: invite.updated_at.into(),
        }
    }
}

/// Response for listing workspace invitations.
pub type Invites = Vec<Invite>;

/// Response containing a generated shareable invite code.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InviteCode {
    /// The generated invite code that can be shared.
    pub invite_code: String,
    /// ID of the workspace this invite code is for.
    pub workspace_id: Uuid,
    /// Role assigned when someone joins via this code.
    pub role: WorkspaceRole,
    /// When the invite code expires.
    pub expires_at: Timestamp,
}

impl InviteCode {
    /// Creates a new invite code response from a workspace invite.
    pub fn from_invite(invite: &model::WorkspaceInvite) -> Self {
        Self {
            invite_code: invite.invite_token.clone(),
            workspace_id: invite.workspace_id,
            role: invite.invited_role,
            expires_at: invite.expires_at.into(),
        }
    }
}
