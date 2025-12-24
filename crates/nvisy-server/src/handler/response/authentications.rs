//! Authentication response types.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response returned after successful authentication (login/signup).
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthToken {
    /// ID of the account.
    pub account_id: Uuid,

    /// Display name.
    pub display_name: String,

    /// Email address.
    pub email_address: String,

    /// Timestamp when the token was issued.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires.
    pub expires_at: Timestamp,
}
