//! Authentication request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request payload for login.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "emailAddress": "user@example.com",
    "password": "password123",
    "rememberMe": true
}))]
pub struct Login {
    /// Email address of the account.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: String,

    /// Password of the account.
    #[validate(length(min = 1, max = 1000))]
    pub password: String,

    /// Whether to remember this device for extended session.
    pub remember_me: bool,
}

/// Request payload for signup.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "John Doe",
    "emailAddress": "john.doe@example.com",
    "password": "password123",
    "rememberMe": true,
    "termsAccepted": true
}))]
pub struct Signup {
    /// Display name of the account.
    #[validate(length(min = 2, max = 32))]
    pub display_name: String,

    /// Email address of the account.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: String,

    /// Password of the account.
    #[validate(length(min = 8, max = 128))]
    pub password: String,

    /// Whether to remember the device for extended session.
    pub remember_me: bool,
}

// TODO: Implement password reset

/// Request payload for password reset initiation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "emailAddress": "user@example.com"
}))]
pub struct RequestPasswordReset {
    /// Email address of the account to reset password for.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: String,
}

/// Request payload for password reset confirmation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "token": "abc123def456ghi789jkl012mno345pqr678stu901",
    "newPassword": "NewPassword123",
}))]
pub struct ConfirmPasswordReset {
    /// Password reset token.
    #[validate(length(min = 10, max = 200))]
    pub token: String,

    /// New password.
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

/// Request payload for email verification.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "token": "abc123def456ghi789jkl012mno345pqr678stu901"
}))]
pub struct VerifyEmail {
    /// Email verification token.
    #[validate(length(min = 10, max = 200))]
    pub token: String,
}

/// Request payload for resending email verification.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "emailAddress": "user@example.com"
}))]
pub struct ResendEmailVerification {
    /// Email address to resend verification to.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: String,
}
