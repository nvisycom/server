//! Authentication response types.

use jiff::Timestamp;
use nvisy_postgres::types::Username;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Response returned after successful authentication (login/signup).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthToken {
    /// The JWT API token for authentication.
    pub api_token: String,
    /// Handle of the authenticated account.
    pub username: Username,
    /// Timestamp when the token was issued.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires.
    pub expires_at: Timestamp,
}
