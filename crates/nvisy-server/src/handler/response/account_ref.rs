//! Shared account reference for response types.

use nvisy_postgres::types::{AccountRefRow, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Public reference to the account behind a resource — whoever created it,
/// uploaded it, triggered it, or performed it.
///
/// Reused across resource responses so an account is always presented the same
/// way: its immutable id (the durable reference) plus a handle and optional
/// avatar for display.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountRef {
    /// Immutable id of the account.
    pub id: Uuid,
    /// Handle of the account (display).
    pub username: Handle,
    /// Human-readable display name, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Serve path of the account's avatar, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

impl AccountRef {
    /// Builds a reference from a resolved id, handle, display name, and avatar
    /// path.
    pub fn new(
        id: Uuid,
        username: Handle,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Self {
        Self {
            id,
            username,
            display_name,
            avatar_url,
        }
    }
}

impl From<AccountRefRow> for AccountRef {
    fn from(row: AccountRefRow) -> Self {
        Self::new(row.id, row.username, row.display_name, row.avatar_url)
    }
}
