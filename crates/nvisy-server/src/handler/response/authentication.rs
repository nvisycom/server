//! Authentication response types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Response returned after successful authentication (login/signup).
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthToken {
    /// ID of the account.
    pub account_id: Uuid,

    /// Display name.
    pub display_name: String,

    /// Email address.
    pub email_address: String,

    /// Timestamp when the token was issued.
    pub issued_at: time::OffsetDateTime,
    /// Timestamp when the token expires.
    pub expires_at: time::OffsetDateTime,
}
