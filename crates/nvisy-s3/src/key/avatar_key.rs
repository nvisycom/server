//! `{id}_{version}` keys for account and workspace avatars.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use super::bucket::Bucket;
use super::object_key::{ObjectKey, parse_error, split_id_version};
use crate::error::{Error, Result};

/// A validated key for an account-scoped object (an avatar).
///
/// The key format is `account_{account_id}_{version}`, where `version` is a
/// content hash. Each avatar version is a distinct object, so a versioned URL
/// maps to immutable bytes and a stale version simply does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountAvatarKey {
    pub account_id: Uuid,
    pub version: String,
}

impl ObjectKey for AccountAvatarKey {
    const BUCKET: Bucket = Bucket::AccountAvatars;
    const PREFIX: &'static str = "account_";
}

impl AccountAvatarKey {
    /// Creates a new account key for a specific avatar version.
    pub fn new(account_id: Uuid, version: impl Into<String>) -> Self {
        Self {
            account_id,
            version: version.into(),
        }
    }
}

impl fmt::Display for AccountAvatarKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}_{}", Self::PREFIX, self.account_id, self.version)
    }
}

impl FromStr for AccountAvatarKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (id, version) = split_id_version::<Self>(s)?;
        let account_id =
            Uuid::parse_str(id).map_err(|e| parse_error(format!("Invalid account UUID: {e}")))?;
        Ok(Self::new(account_id, version))
    }
}

/// A validated key for a workspace-scoped object (an avatar/logo).
///
/// The key format is `workspace_{workspace_id}_{version}`, where `version` is a
/// content hash. Each avatar version is a distinct object, so a versioned URL
/// maps to immutable bytes and a stale version simply does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceAvatarKey {
    pub workspace_id: Uuid,
    pub version: String,
}

impl ObjectKey for WorkspaceAvatarKey {
    const BUCKET: Bucket = Bucket::WorkspaceAvatars;
    const PREFIX: &'static str = "workspace_";
}

impl WorkspaceAvatarKey {
    /// Creates a new workspace key for a specific avatar version.
    pub fn new(workspace_id: Uuid, version: impl Into<String>) -> Self {
        Self {
            workspace_id,
            version: version.into(),
        }
    }
}

impl fmt::Display for WorkspaceAvatarKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}_{}", Self::PREFIX, self.workspace_id, self.version)
    }
}

impl FromStr for WorkspaceAvatarKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let (id, version) = split_id_version::<Self>(s)?;
        let workspace_id =
            Uuid::parse_str(id).map_err(|e| parse_error(format!("Invalid workspace UUID: {e}")))?;
        Ok(Self::new(workspace_id, version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_avatar_round_trips() {
        let account_id = Uuid::new_v4();
        let key = AccountAvatarKey::new(account_id, "abc123");
        assert_eq!(key.to_string(), format!("account_{account_id}_abc123"));
        let decoded: AccountAvatarKey = key.to_string().parse().unwrap();
        assert_eq!(decoded.account_id, account_id);
        assert_eq!(decoded.version, "abc123");
    }

    #[test]
    fn workspace_avatar_round_trips() {
        let workspace_id = Uuid::new_v4();
        let key = WorkspaceAvatarKey::new(workspace_id, "v9");
        let decoded: WorkspaceAvatarKey = key.to_string().parse().unwrap();
        assert_eq!(decoded.workspace_id, workspace_id);
        assert_eq!(decoded.version, "v9");
    }

    #[test]
    fn account_avatar_rejects_bad_input() {
        assert!(AccountAvatarKey::from_str("file_abc").is_err());
        assert!(AccountAvatarKey::from_str("account_not-a-uuid").is_err());
    }
}
