//! Project invite response types.

use nvisy_postgres::model::ProjectInvite;
use nvisy_postgres::types::{InviteStatus, ProjectRole};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Response returned when a project invite is successfully created.
///
/// This response includes all the essential information about the newly created
/// invitation, including the unique invite ID that can be used to track or cancel
/// the invitation later.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteResponse {
    /// Unique identifier of the created invitation.
    ///
    /// This ID can be used to cancel the invitation or track its status.
    pub invite_id: Uuid,

    /// ID of the project the invitation is for.
    ///
    /// The invitee will become a member of this project if they accept.
    pub project_id: Uuid,

    /// Email address of the invitee.
    ///
    /// The invitation will be sent to this email address. The email is normalized
    /// to lowercase for consistency.
    pub invitee_email: String,

    /// Role the invitee will have if they accept.
    ///
    /// Determines the level of access and permissions the invitee will have
    /// in the project. Common roles include: Owner, Admin, Editor, Viewer.
    pub invited_role: ProjectRole,

    /// Current status of the invitation.
    ///
    /// Possible values: Pending, Accepted, Rejected, Cancelled, Expired.
    /// Newly created invitations start with status Pending.
    pub invite_status: InviteStatus,

    /// When the invitation will expire.
    ///
    /// After this timestamp, the invitation can no longer be accepted.
    /// The expiration period is configurable when creating the invite (1-30 days).
    pub expires_at: OffsetDateTime,

    /// When the invitation was created.
    ///
    /// UTC timestamp of when this invitation record was created.
    pub created_at: OffsetDateTime,
}

impl From<ProjectInvite> for CreateInviteResponse {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            project_id: invite.project_id,
            invitee_email: invite.invitee_email,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at,
            created_at: invite.created_at,
        }
    }
}

/// Represents a project invitation in list responses.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListInvitesResponseItem {
    /// Unique identifier of the invitation.
    pub invite_id: Uuid,
    /// Email address of the invitee.
    pub invitee_email: String,
    /// Account ID if the invitee has an account.
    pub invitee_id: Option<Uuid>,
    /// Role the invitee will have if they accept.
    pub invited_role: ProjectRole,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation expires.
    pub expires_at: OffsetDateTime,
    /// When the invitation was created.
    pub created_at: OffsetDateTime,
    /// When the invitation was last updated.
    pub updated_at: OffsetDateTime,
}

impl From<ProjectInvite> for ListInvitesResponseItem {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            invitee_email: invite.invitee_email,
            invitee_id: invite.invitee_id,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at,
            created_at: invite.created_at,
            updated_at: invite.updated_at,
        }
    }
}

/// Response for listing project invitations.
///
/// Contains a paginated list of all invitations for a specific project,
/// including their current status and metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListInvitesResponse {
    /// ID of the project these invitations belong to.
    pub project_id: Uuid,

    /// List of project invitations.
    ///
    /// This list contains all invitations matching the query, subject to
    /// pagination limits. Each item includes the invitation status and
    /// details about the invitee.
    pub invites: Vec<ListInvitesResponseItem>,

    /// Total count of invitations for this project.
    ///
    /// This count represents all invitations, not just the current page.
    /// Use this value to implement pagination controls in the UI.
    pub total_count: usize,
}

/// Response for invitation reply operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplyInviteResponse {
    /// ID of the invitation.
    pub invite_id: Uuid,
    /// ID of the project.
    pub project_id: Uuid,
    /// Email address of the invitee.
    pub invitee_email: String,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation was accepted or declined.
    pub updated_at: OffsetDateTime,
}

impl From<ProjectInvite> for ReplyInviteResponse {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            project_id: invite.project_id,
            invitee_email: invite.invitee_email,
            invite_status: invite.invite_status,
            updated_at: invite.updated_at,
        }
    }
}

/// Response for invitation cancellation operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CancelInviteResponse {
    /// ID of the cancelled invitation.
    pub invite_id: Uuid,
    /// ID of the project.
    pub project_id: Uuid,
    /// Email address of the invitee.
    pub invitee_email: String,
    /// Reason for cancellation.
    pub status_reason: Option<String>,
}

impl From<ProjectInvite> for CancelInviteResponse {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            project_id: invite.project_id,
            invitee_email: invite.invitee_email,
            status_reason: invite.status_reason,
        }
    }
}
