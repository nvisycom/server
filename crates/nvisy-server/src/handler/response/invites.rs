//! Workspace invite response types.

use jiff::Timestamp;
use nvisy_postgres::model::{self, WorkspaceInvite};
use nvisy_postgres::types::{InviteStatus, Slug, WorkspaceRole};
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
pub struct Invite {
    /// Unique identifier of the invitation.
    pub invite_id: Uuid,
    /// Slug of the workspace the invitation is for.
    pub workspace_slug: Slug,
    /// Email address of the invitee (omitted for open invite codes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invitee_email: Option<String>,
    /// Invite token (only included for open invitations without invitee_email).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite_token: Option<String>,
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

impl Invite {
    pub fn from_model(invite: WorkspaceInvite, workspace_slug: Slug) -> Self {
        // Only include invite_token for open invitations (no invitee_email)
        let invite_token = if invite.invitee_email.is_none() {
            Some(invite.invite_token.clone())
        } else {
            None
        };

        Self {
            invite_id: invite.id,
            workspace_slug,
            invitee_email: invite.invitee_email,
            invite_token,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at.into(),
            created_at: invite.created_at.into(),
            updated_at: invite.updated_at.into(),
        }
    }
}

/// Paginated response for workspace invitations.
pub type InvitesPage = Page<Invite>;

/// Acknowledgement returned after sending a workspace invitation.
///
/// The response is deliberately uniform: it carries no invite identifier or
/// status, so it is identical whether or not the address belonged to a known
/// account and cannot be used to probe for account existence.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InviteSent {
    /// Human-readable confirmation message.
    pub detail: String,
}

impl InviteSent {
    /// Creates the standard invitation acknowledgement.
    pub fn new() -> Self {
        Self {
            detail: "If the address belongs to a user, they have been invited.".to_owned(),
        }
    }
}

impl Default for InviteSent {
    fn default() -> Self {
        Self::new()
    }
}

/// Response containing a generated shareable invite code.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InviteCode {
    /// The generated invite code that can be shared.
    pub invite_code: String,
    /// Slug of the workspace this invite code is for.
    pub workspace_slug: Slug,
    /// Role assigned when someone joins via this code.
    pub role: WorkspaceRole,
    /// When the invite code expires.
    pub expires_at: Timestamp,
}

impl InviteCode {
    /// Creates a new invite code response from a workspace invite.
    pub fn from_invite(invite: &model::WorkspaceInvite, workspace_slug: Slug) -> Self {
        Self {
            invite_code: invite.invite_token.clone(),
            workspace_slug,
            role: invite.invited_role,
            expires_at: invite.expires_at.into(),
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
    /// Slug of the workspace.
    pub workspace_slug: Slug,
    /// Display name of the workspace.
    pub display_name: String,
    /// Description of the workspace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags associated with the workspace.
    pub tags: Vec<String>,
    /// Role the user will have if they join.
    pub invited_role: WorkspaceRole,
    /// Timestamp when the workspace was created.
    pub created_at: Timestamp,
    /// When the invite expires.
    pub expires_at: Timestamp,
}

impl InvitePreview {
    /// Creates an invite preview from workspace and invite models.
    pub fn from_models(workspace: model::Workspace, invite: model::WorkspaceInvite) -> Self {
        let tags = workspace.get_tags();
        Self {
            workspace_slug: workspace.slug,
            display_name: workspace.display_name,
            description: workspace.description,
            tags,
            invited_role: invite.invited_role,
            created_at: workspace.created_at.into(),
            expires_at: invite.expires_at.into(),
        }
    }
}
