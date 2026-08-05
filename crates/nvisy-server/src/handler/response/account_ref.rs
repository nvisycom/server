//! Shared account reference for response types.

use nvisy_postgres::types::{AccountRefRow, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Public reference to the account behind a resource — whoever created it,
/// uploaded it, triggered it, or performed it.
///
/// Reused across resource responses so an account is always presented the same
/// way: a handle plus an optional avatar.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountRef {
    /// Handle of the account.
    pub username: Handle,
    /// Human-readable display name, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Serve path of the account's avatar, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

impl AccountRef {
    /// Builds a reference from a resolved handle, display name, and avatar path.
    pub fn new(username: Handle, display_name: Option<String>, avatar_url: Option<String>) -> Self {
        Self {
            username,
            display_name,
            avatar_url,
        }
    }
}

impl From<AccountRefRow> for AccountRef {
    fn from(row: AccountRefRow) -> Self {
        Self::new(row.username, row.display_name, row.avatar_url)
    }
}
