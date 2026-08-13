//! Deletion policy enumeration for reconciling deleted source objects.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// What an import does with a file whose source object no longer exists.
///
/// Corresponds to the `SYNC_DELETION_POLICY` PostgreSQL enum. Deletion is opt-in
/// per connection: the default `Ignore` keeps imports strictly additive so a
/// transient listing error or a misconfigured root path can never remove files.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::SyncDeletionPolicy"]
pub enum SyncDeletionPolicy {
    /// Leave the imported file untouched when its source object is gone.
    #[db_rename = "ignore"]
    #[serde(rename = "ignore")]
    #[default]
    Ignore,

    /// Delete the imported file when its source object is gone: the file row is
    /// soft-deleted (preserving import provenance) and its stored object is
    /// removed to reclaim storage.
    #[db_rename = "delete"]
    #[serde(rename = "delete")]
    Delete,
}

impl SyncDeletionPolicy {
    /// Returns whether deleted source objects are reconciled at all.
    #[inline]
    pub fn removes_files(self) -> bool {
        matches!(self, SyncDeletionPolicy::Delete)
    }
}

#[cfg(test)]
mod tests {
    use super::SyncDeletionPolicy;

    #[test]
    fn default_is_ignore_and_never_removes() {
        assert_eq!(SyncDeletionPolicy::default(), SyncDeletionPolicy::Ignore);
        assert!(!SyncDeletionPolicy::Ignore.removes_files());
        assert!(SyncDeletionPolicy::Delete.removes_files());
    }

    #[test]
    fn serde_uses_snake_case() {
        let json = serde_json::to_string(&SyncDeletionPolicy::Delete).unwrap();
        assert_eq!(json, "\"delete\"");
        let parsed: SyncDeletionPolicy = serde_json::from_str("\"ignore\"").unwrap();
        assert_eq!(parsed, SyncDeletionPolicy::Ignore);
    }
}
