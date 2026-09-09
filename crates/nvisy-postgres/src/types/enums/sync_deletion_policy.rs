//! Deletion policy enumeration for reconciling deleted source objects.

use super::db_enum;

db_enum! {
    /// What a whole-listing import does with a file whose source object no longer
    /// exists.
    ///
    /// Corresponds to the `SYNC_DELETION_POLICY` PostgreSQL enum. Only
    /// whole-listing import reconciles deletions (it compares the full source
    /// listing against what was imported), so this applies to object-store import
    /// alone; picker-driven file-service import transfers only the files the user
    /// selected and never reconciles. Deletion is opt-in per connection: the
    /// default `Ignore` keeps imports strictly additive so a transient listing
    /// error or a misconfigured root path can never remove files.
    pub enum SyncDeletionPolicy: Default = Ignore, "crate::schema::sql_types::SyncDeletionPolicy" {
        /// Leave the imported file untouched when its source object is gone.
        Ignore = "ignore",
        /// Delete the imported file when its source object is gone: the file row
        /// is soft-deleted (preserving import provenance) and its stored object is
        /// removed to reclaim storage.
        Delete = "delete",
    }
}
