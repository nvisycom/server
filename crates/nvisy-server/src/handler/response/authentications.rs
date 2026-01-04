//! Authentication response types.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response returned after successful authentication (login/signup).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthToken {
    /// The JWT API token for authentication.
    pub api_token: String,
    /// ID of the authenticated account.
    pub account_id: Uuid,
    /// ID of the token.
    pub token_id: Uuid,
    /// Timestamp when the token was issued.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires.
    pub expires_at: Timestamp,
}
