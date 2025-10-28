//! Project invite request types.

use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request payload for creating a new project invite.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "inviteeEmail": "colleague@example.com",
    "invitedRole": "Editor",
    "inviteMessage": "Join our project to collaborate on documents!",
    "expiresInDays": 7
}))]
pub struct CreateInviteRequest {
    /// Email address of the person to invite.
    #[validate(email, length(max = 254))]
    pub invitee_email: String,
    /// Role the invitee will have if they accept the invitation.
    #[serde(default = "default_invite_role")]
    pub invited_role: ProjectRole,
    /// Optional personal message to include with the invitation.
    ///
    /// This message will be included in the invitation email. The content is
    /// validated to prevent XSS and injection attacks.
    #[validate(length(max = 1000), custom(function = "validate_safe_text"))]
    #[serde(default)]
    pub invite_message: String,
    /// Number of days until the invitation expires (1-30 days, default: 7).
    #[validate(range(min = 1, max = 30))]
    #[serde(default = "default_expiry_days")]
    pub expires_in_days: u8,
}

fn default_invite_role() -> ProjectRole {
    ProjectRole::Editor
}

fn default_expiry_days() -> u8 {
    7
}

/// Validates that text content is safe and doesn't contain potential XSS or injection attacks.
///
/// This function performs basic sanitization checks to prevent common security issues.
fn validate_safe_text(text: &str) -> Result<(), validator::ValidationError> {
    // Check for script tags
    if text.to_lowercase().contains("<script") {
        return Err(validator::ValidationError::new("contains_script_tag"));
    }

    // Check for common XSS patterns
    if text.contains("javascript:") || text.contains("data:text/html") {
        return Err(validator::ValidationError::new("contains_xss_pattern"));
    }

    // Check for SQL injection patterns (basic check)
    let suspicious_patterns = ["--", "/*", "*/", "xp_", "sp_", "exec(", "execute("];
    for pattern in &suspicious_patterns {
        if text.to_lowercase().contains(pattern) {
            return Err(validator::ValidationError::new(
                "contains_suspicious_pattern",
            ));
        }
    }

    Ok(())
}

/// Request to respond to a project invitation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "acceptInvite": true
}))]
pub struct ReplyInviteRequest {
    /// Whether to accept or decline the invitation.
    pub accept_invite: bool,
}
