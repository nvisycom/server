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
    "password": "SecurePassword123!",
    "rememberMe": true
}))]
pub struct LoginRequest {
    /// Email address of the account.
    #[validate(email)]
    pub email_address: String,
    /// Password of the account.
    pub password: String,
    /// Whether to remember this device.
    pub remember_me: bool,
}

/// Request payload for signup.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "John Doe",
    "emailAddress": "john.doe@example.com",
    "password": "SecurePassword123!",
    "rememberMe": true
}))]
pub struct SignupRequest {
    /// Display name of the account.
    #[validate(length(min = 2, max = 32))]
    pub display_name: String,
    /// Email address of the account.
    #[validate(email)]
    pub email_address: String,
    /// Password of the account.
    pub password: String,
    /// Whether to remember the device.
    pub remember_me: bool,
}
