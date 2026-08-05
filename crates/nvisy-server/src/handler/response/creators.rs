//! Shared creator/actor identity for response types.

use nvisy_postgres::types::{CreatorRow, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Public identity of the account behind a resource — its creator, or whoever
/// triggered an action.
///
/// Reused across resource responses so a creator (or actor) is always presented
/// the same way: a handle plus an optional avatar.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Creator {
    /// Handle of the account.
    pub username: Handle,
    /// Serve path of the account's avatar, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

impl Creator {
    /// Builds a creator from a resolved handle and optional avatar path.
    pub fn new(username: Handle, avatar_url: Option<String>) -> Self {
        Self {
            username,
            avatar_url,
        }
    }
}

impl From<CreatorRow> for Creator {
    fn from(row: CreatorRow) -> Self {
        Self::new(row.username, row.avatar_url)
    }
}
