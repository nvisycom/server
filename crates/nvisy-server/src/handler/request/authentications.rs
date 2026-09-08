//! Authentication request types.

use nvisy_postgres::types::Handle;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request payload for login.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Login {
    /// Email address or username of the account.
    #[validate(length(min = 3, max = 254))]
    pub identifier: String,
    /// Password of the account.
    #[validate(length(min = 1, max = 1000))]
    pub password: String,
    /// Whether to remember this device for extended session. Defaults to false.
    #[serde(default)]
    pub remember_me: bool,
}

/// Request payload for signup.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Signup {
    /// Public account handle, unique across all accounts.
    pub username: Handle,

    /// Optional display name of the account.
    #[validate(length(min = 2, max = 32))]
    pub display_name: Option<String>,

    /// Email address of the account.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: String,

    /// Password of the account.
    #[validate(length(min = 8, max = 128))]
    pub password: String,

    /// Whether to remember the device for extended session. Defaults to false.
    #[serde(default)]
    pub remember_me: bool,
}

// TODO: Implement password reset

/// Request payload for password reset initiation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestPasswordReset {
    /// Email address of the account to reset password for.
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: String,
}

/// Request payload for password reset confirmation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmPasswordReset {
    /// Password reset token.
    #[validate(length(min = 10, max = 200))]
    pub token: String,

    /// New password.
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

/// Query parameters the provider appends when redirecting to the OIDC callback.
///
/// The callback serves every OIDC flow (sign-in, link, and step-up reauth); the
/// stashed state selects which. On success the provider sends `code` + `state`;
/// on denial it sends `error` (and `state`) with no `code`, so `code` is optional
/// and the handler treats a missing code or a present error as a failed flow.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OidcCallbackQuery {
    /// The authorization code to exchange for tokens; absent on a denial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The opaque CSRF state echoed back; must match a pending flow.
    pub state: String,
    /// The provider's error code when the user denied or the flow failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
