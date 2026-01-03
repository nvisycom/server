//! Workspace invite request types.

use nvisy_postgres::types::{InviteFilter, InviteSortBy, SortOrder, WorkspaceRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new workspace invite.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateInvite {
    /// Email address of the person to invite.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub invitee_email: String,
    /// Role the invitee will have if they accept the invitation.
    pub invited_role: WorkspaceRole,
    /// When the invitation expires.
    pub expires_in: InviteExpiration,
}

impl CreateInvite {
    /// Converts to database model.
    pub fn into_model(
        self,
        workspace_id: Uuid,
        created_by: Uuid,
    ) -> nvisy_postgres::model::NewWorkspaceInvite {
        nvisy_postgres::model::NewWorkspaceInvite {
            workspace_id,
            invitee_email: Some(self.invitee_email),
            invited_role: Some(self.invited_role),
            expires_at: self.expires_in.to_expiry_timestamp().map(Into::into),
            created_by,
            updated_by: created_by,
            ..Default::default()
        }
    }
}

/// Request to respond to a workspace invitation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ReplyInvite {
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
pub struct GenerateInviteCode {
    /// Role to assign when someone joins via this invite code.
    pub invited_role: WorkspaceRole,
    /// When the invite code expires.
    pub expires_in: InviteExpiration,
}

impl GenerateInviteCode {
    /// Converts to database model.
    pub fn into_model(
        self,
        workspace_id: Uuid,
        created_by: Uuid,
    ) -> nvisy_postgres::model::NewWorkspaceInvite {
        nvisy_postgres::model::NewWorkspaceInvite {
            workspace_id,
            invitee_email: None,
            invited_role: Some(self.invited_role),
            expires_at: self.expires_in.to_expiry_timestamp().map(Into::into),
            created_by,
            updated_by: created_by,
            ..Default::default()
        }
    }
}

/// Query parameters for listing workspace invites.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListInvitesQuery {
    /// Filter by invited role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<WorkspaceRole>,
    /// Sort by field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<InviteSortField>,
    /// Sort order (asc or desc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<SortOrder>,
}

/// Fields to sort invites by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InviteSortField {
    /// Sort by invitee email.
    Email,
    /// Sort by creation date.
    Date,
}

impl ListInvitesQuery {
    /// Converts to filter model.
    pub fn to_filter(&self) -> InviteFilter {
        InviteFilter { role: self.role }
    }

    /// Converts to sort model.
    pub fn to_sort(&self) -> InviteSortBy {
        let order = self.order.unwrap_or_default();
        match self.sort_by {
            Some(InviteSortField::Email) => InviteSortBy::Email(order),
            Some(InviteSortField::Date) | None => InviteSortBy::Date(order),
        }
    }
}
