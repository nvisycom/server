//! Sync status enumeration for connection synchronization operations.

use super::db_enum;

db_enum! {
    /// Defines the execution status of a connection sync run.
    ///
    /// Corresponds to the `SYNC_STATUS` PostgreSQL enum and tracks the state of an
    /// individual synchronization run.
    pub enum SyncStatus: Default = Pending, "crate::schema::sql_types::SyncStatus" {
        /// Sync is queued.
        Pending = "pending",
        /// Sync is in progress.
        Running = "running",
        /// Sync finished successfully.
        Completed = "completed",
        /// Sync failed with error.
        Failed = "failed",
        /// Sync was cancelled.
        Cancelled = "cancelled",
    }
}

impl SyncStatus {
    /// Returns whether the sync failed.
    #[inline]
    pub fn is_failed(self) -> bool {
        matches!(self, SyncStatus::Failed)
    }

    /// Returns whether the sync is in progress (pending or running).
    #[inline]
    pub fn is_in_progress(self) -> bool {
        matches!(self, SyncStatus::Pending | SyncStatus::Running)
    }
}
