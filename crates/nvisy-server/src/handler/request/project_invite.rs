//! Project invite request types.

use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use super::validation::validation_error;

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
pub struct CreateInvite {
    /// Email address of the person to invite.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    #[validate(custom(function = "validate_invitee_email"))]
    pub invitee_email: String,
    /// Role the invitee will have if they accept the invitation.
    #[serde(default)]
    pub invited_role: ProjectRole,
    /// Optional personal message to include with the invitation.
    ///
    /// This message will be included in the invitation email. The content is
    /// validated to prevent XSS and injection attacks.
    #[validate(length(max = 1000))]
    #[validate(custom(function = "validate_safe_text"))]
    #[serde(default)]
    pub invite_message: String,
    /// Number of days until the invitation expires (1-30 days, default: 7).
    #[validate(range(min = 1, max = 30))]
    pub expires_in_days: Option<u8>,
}

// Validation functions using consolidated utilities

/// Blocked email domains for invitations.
const BLOCKED_EMAIL_DOMAINS: &[&str] = &[
    "10minutemail.com",
    "guerrillamail.com",
    "mailinator.com",
    "tempmail.org",
    "throwaway.email",
];

/// Validates invitee email format and domain restrictions.
fn validate_invitee_email(email: &str) -> Result<(), ValidationError> {
    let normalized = email.trim().to_lowercase();

    // Basic email validation
    if normalized.is_empty() {
        return Err(validation_error(
            "email_empty",
            "Email address cannot be empty",
        ));
    }
    if normalized.len() < 5 {
        return Err(validation_error(
            "email_too_short",
            "Email address must be at least 5 characters long",
        ));
    }
    if normalized.len() > 254 {
        return Err(validation_error(
            "email_too_long",
            "Email address cannot exceed 254 characters",
        ));
    }
    if !normalized.contains('@') || !normalized.contains('.') {
        return Err(validation_error(
            "email_invalid_format",
            "Please provide a valid email address",
        ));
    }

    // Check against blocked domains
    if let Some(domain) = normalized.split('@').nth(1) {
        if BLOCKED_EMAIL_DOMAINS.contains(&domain) {
            return Err(validation_error(
                "blocked_domain",
                "This email domain is not allowed for invitations",
            ));
        }

        // Check for suspicious patterns
        if domain.contains("temp") || domain.contains("disposable") || domain.contains("fake") {
            return Err(validation_error(
                "suspicious_domain",
                "Suspicious or temporary email domains are not allowed",
            ));
        }
    }

    Ok(())
}

/// Validates that text content is safe and doesn't contain potential XSS or injection attacks.
fn validate_safe_text(text: &str) -> Result<(), ValidationError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(()); // Empty is okay for optional invite message
    }

    Ok(())
}

/// Request to respond to a project invitation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "acceptInvite": true,
    "message": "Thank you for the invitation!"
}))]
pub struct ReplyInvite {
    /// Whether to accept or decline the invitation.
    pub accept_invite: bool,

    /// Optional message when responding to invitation.
    #[validate(length(max = 300))]
    pub message: Option<String>,
}
