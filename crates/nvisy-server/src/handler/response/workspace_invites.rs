//! Workspace invite response types.

use jiff::Timestamp;
use nvisy_postgres::model::{self, WorkspaceInvite as WorkspaceInviteModel};
use nvisy_postgres::types::{Handle, InviteStatus, WorkspaceRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Workspace invite with complete information.
///
/// This response includes all the essential information about an
/// invitation, including the unique invite ID that can be used to track or cancel
/// the invitation later.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInvite {
    /// Unique identifier of the invitation.
    pub invite_id: Uuid,
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// URL-safe workspace handle. Display-only.
    pub workspace_handle: Handle,
    /// Email address of the invitee (omitted for open invite codes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invitee_email: Option<String>,
    /// Role the invitee will have if they accept.
    pub invited_role: WorkspaceRole,
    /// Current status of the invitation.
    ///
    /// For a single-use open code, `accepted` means the code has been consumed;
    /// `pending` (and unexpired) means it is still active.
    pub invite_status: InviteStatus,
    /// When the invitation expires.
    pub expires_at: Timestamp,
    /// When the invitee responded (accepted or declined), if they have. For a
    /// consumed single-use code, this is when it was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responded_at: Option<Timestamp>,
    /// When the invitation was created.
    pub created_at: Timestamp,
    /// When the invitation was last updated.
    pub updated_at: Timestamp,
}

impl WorkspaceInvite {
    /// Builds the invite summary. The invite token is deliberately absent: it is
    /// delivered only by the dedicated generate-invite-code endpoint, never in a
    /// list or detail response.
    pub fn from_model(
        invite: WorkspaceInviteModel,
        workspace_id: Uuid,
        workspace_handle: Handle,
    ) -> Self {
        Self {
            invite_id: invite.id,
            workspace_id,
            workspace_handle,
            invitee_email: invite.invitee_email,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at.into(),
            responded_at: invite.responded_at.map(Into::into),
            created_at: invite.created_at.into(),
            updated_at: invite.updated_at.into(),
        }
    }
}

/// Paginated response for workspace invitations.
pub type WorkspaceInvitesPage = Page<WorkspaceInvite>;

/// Acknowledgement returned after sending a workspace invitation.
///
/// The response is deliberately uniform: it carries no invite identifier or
/// status, so it is identical whether or not the address belonged to a known
/// account and cannot be used to probe for account existence.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteSent {
    /// Human-readable confirmation message.
    pub detail: String,
}

impl WorkspaceInviteSent {
    /// Creates the standard invitation acknowledgement.
    pub fn new() -> Self {
        Self {
            detail: "If the address belongs to a user, they have been invited.".to_owned(),
        }
    }
}

impl Default for WorkspaceInviteSent {
    fn default() -> Self {
        Self::new()
    }
}

/// Response for a freshly generated shareable invite code: the full invite plus
/// its raw, show-once code.
///
/// The `inviteCode` is a single-use bearer secret returned **only here**, never
/// from the list or any other endpoint — treat it like the webhook signing
/// secret and store it at once. The flattened invite fields (notably `inviteId`
/// and `inviteStatus`) let a client correlate this code to its listing row and
/// track its lifecycle: once redeemed the code is consumed, and the row's
/// `inviteStatus` becomes `accepted` (with `respondedAt` set) — so a client can
/// tell an active code from a spent one without ever re-fetching the secret.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteCode {
    /// The generated invite code to share. Shown once and never returned again.
    pub invite_code: String,
    /// The invite this code redeems, in the same shape the listing returns.
    #[serde(flatten)]
    pub invite: WorkspaceInvite,
}

impl WorkspaceInviteCode {
    /// Creates an invite-code response from a freshly created open invite,
    /// pairing its show-once token with the full invite summary.
    pub fn from_invite(
        invite: model::WorkspaceInvite,
        workspace_id: Uuid,
        workspace_handle: Handle,
    ) -> Self {
        Self {
            invite_code: invite.invite_token.clone(),
            invite: WorkspaceInvite::from_model(invite, workspace_id, workspace_handle),
        }
    }
}

/// Preview of an invite with workspace details for display before joining.
///
/// This is a public-facing response that shows workspace information
/// to help users decide whether to join via an invite code.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// URL-safe workspace handle. Display-only.
    pub workspace_handle: Handle,
    /// Display name of the workspace.
    pub display_name: String,
    /// Description of the workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Role the user will have if they join.
    pub invited_role: WorkspaceRole,
    /// Timestamp when the workspace was created.
    pub created_at: Timestamp,
    /// When the invite expires.
    pub expires_at: Timestamp,
}

impl InvitePreview {
    /// Creates an invite preview from workspace and invite models.
    pub fn from_models(workspace: model::Workspace, invite: &model::WorkspaceInvite) -> Self {
        Self {
            workspace_id: workspace.id,
            workspace_handle: workspace.handle,
            display_name: workspace.display_name,
            description: workspace.description,
            invited_role: invite.invited_role,
            created_at: workspace.created_at.into(),
            expires_at: invite.expires_at.into(),
        }
    }
}
