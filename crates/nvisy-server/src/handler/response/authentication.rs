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
    /// Regional policy.
    pub data_collection: bool,

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

    /// Region policy of the account.
    pub regional_policy: String,
    /// Display name of the account.
    pub display_name: String,
    /// Email address of the account.
    pub email_address: String,

    /// Timestamp when the token was issued.
    pub issued_at: time::OffsetDateTime,
    /// Timestamp when the token expires.
    pub expired_at: time::OffsetDateTime,
}
