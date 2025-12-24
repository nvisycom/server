//! Project invite request types.

use nvisy_postgres::types::ProjectRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request payload for creating a new project invite.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateInvite {
    /// Email address of the person to invite.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub invitee_email: String,
    /// Role the invitee will have if they accept the invitation.
    #[serde(default)]
    pub invited_role: ProjectRole,
    /// Optional personal message to include with the invitation.
    ///
    /// This message will be included in the invitation email. The content is
    /// validated to prevent XSS and injection attacks.
    #[validate(length(max = 1000))]
    #[serde(default)]
    pub invite_message: String,
    /// Number of days until the invitation expires (1-30 days, default: 7).
    #[validate(range(min = 1, max = 30))]
    pub expires_in_days: Option<u8>,
}

/// Request to respond to a project invitation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ReplyInvite {
    /// Whether to accept or decline the invitation.
    pub accept_invite: bool,

    /// Optional message when responding to invitation.
    #[validate(length(max = 300))]
    pub message: Option<String>,
}

/// Expiration options for invite codes.
#[must_use]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum InviteExpiration {
    /// Invite code never expires.
    Never,
    /// Expires in 24 hours.
    In24Hours,
    /// Expires in 7 days.
    #[default]
    In7Days,
    /// Expires in 30 days.
    In30Days,
}

impl InviteExpiration {
    /// Returns the duration until expiration, or None if never expires.
    pub fn to_span(self) -> Option<jiff::Span> {
        match self {
            Self::Never => None,
            Self::In24Hours => Some(jiff::Span::new().hours(24)),
            Self::In7Days => Some(jiff::Span::new().days(7)),
            Self::In30Days => Some(jiff::Span::new().days(30)),
        }
    }

    /// Returns the expiry timestamp from now, or None if never expires.
    pub fn to_expiry_timestamp(self) -> Option<jiff::Timestamp> {
        self.to_span().map(|span| jiff::Timestamp::now() + span)
    }
}

/// Request to generate a shareable invite code for a project.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct GenerateInviteCode {
    /// Role to assign when someone joins via this invite code.
    #[serde(default)]
    pub role: ProjectRole,

    /// When the invite code expires.
    #[serde(default)]
    pub expires: InviteExpiration,
}
