//! Deletion policy enumeration for reconciling deleted source objects.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// What a whole-listing import does with a file whose source object no longer
/// exists.
///
/// Corresponds to the `SYNC_DELETION_POLICY` PostgreSQL enum. Only whole-listing
/// import reconciles deletions (it compares the full source listing against what
/// was imported), so this applies to object-store import alone; picker-driven
/// file-service import transfers only the files the user selected and never
/// reconciles. Deletion is opt-in per connection: the default `Ignore` keeps
/// imports strictly additive so a transient listing error or a misconfigured
/// root path can never remove files.
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
