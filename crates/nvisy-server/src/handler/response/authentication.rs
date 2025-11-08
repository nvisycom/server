//! Authentication response types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Response returned after successful login.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    /// ID of the account.
    pub account_id: Uuid,

    /// Timestamp when the token was issued.
    pub issued_at: time::OffsetDateTime,
    /// Timestamp when the token expires.
    pub expires_at: time::OffsetDateTime,
}

/// Response returned after successful signup.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignupResponse {
    /// ID of the account.
    pub account_id: Uuid,

    /// Display name of the account.
    pub display_name: String,
    /// Email address of the account.
    pub email_address: String,

    /// Timestamp when the token was issued.
    pub issued_at: time::OffsetDateTime,
    /// Timestamp when the token expires.
    pub expired_at: time::OffsetDateTime,
}
