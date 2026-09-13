//! Workspace invite request types.

use garde::Validate;
use nvisy_postgres::types::{
    Direction, InviteFilter, InviteSortBy, InviteSortField, WorkspaceRole,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::input::{CreateInviteInput, GenerateInviteCodeInput};

/// Request payload for creating a new workspace invite.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateWorkspaceInvite {
    /// Email address of the person to invite.
    #[garde(email, length(chars, min = 5, max = 254))]
    pub invitee_email: String,
    /// Role the invitee will have if they accept the invitation.
    pub invited_role: WorkspaceRole,
    /// When the invitation expires.
    pub expires_in: InviteExpiration,
}

impl From<CreateWorkspaceInvite> for CreateInviteInput {
    fn from(request: CreateWorkspaceInvite) -> Self {
        CreateInviteInput {
            invitee_email: request.invitee_email,
            invited_role: request.invited_role,
            expires_at: request.expires_in.to_expiry_timestamp(),
        }
    }
}

/// Request to respond to a workspace invitation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct ReplyWorkspaceInvite {
    /// Whether to accept or decline the invitation.
    pub accept_invite: bool,
}

/// Expiration options for invite codes.
#[must_use]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InviteExpiration {
    /// Expires in 24 hours.
    In24Hours,
    /// Expires in 7 days.
    #[default]
    In7Days,
    /// Expires in 30 days.
    In30Days,
}

impl InviteExpiration {
    /// Returns the duration until expiration in hours.
    ///
    /// Uses hours instead of days because `jiff::Timestamp` only supports
    /// units of hours or smaller for arithmetic operations.
    pub fn to_span(self) -> jiff::Span {
        match self {
            Self::In24Hours => jiff::Span::new().hours(24),
            Self::In7Days => jiff::Span::new().hours(7 * 24),
            Self::In30Days => jiff::Span::new().hours(30 * 24),
        }
    }

    /// Returns the expiry timestamp from now.
    pub fn to_expiry_timestamp(self) -> Option<jiff::Timestamp> {
        jiff::Timestamp::now().checked_add(self.to_span()).ok()
    }
}

/// Request to generate a shareable invite code for a workspace.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct GenerateWorkspaceInviteCode {
    /// Role to assign when someone joins via this invite code.
    pub invited_role: WorkspaceRole,
    /// When the invite code expires.
    pub expires_in: InviteExpiration,
}

impl From<GenerateWorkspaceInviteCode> for GenerateInviteCodeInput {
    fn from(request: GenerateWorkspaceInviteCode) -> Self {
        GenerateInviteCodeInput {
            invited_role: request.invited_role,
            expires_at: request.expires_in.to_expiry_timestamp(),
        }
    }
}

/// Query parameters for listing workspace invites.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkspaceInvites {
    /// Filter by invited role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,
    /// Sort by field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<InviteSortField>,
    /// Sort order (asc or desc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Direction>,
}

impl ListWorkspaceInvites {
    /// Converts to filter model.
    pub fn to_filter(&self) -> InviteFilter {
        InviteFilter { role: self.role }
    }

    /// Converts to sort model.
    pub fn to_sort(&self) -> InviteSortBy {
        let order = self.order.unwrap_or_default();
        let field = self.sort_by.unwrap_or_default();
        InviteSortBy::new(field, order)
    }
}
