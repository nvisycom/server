//! Sync status enumeration for connection synchronization operations.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the status of a connection sync operation.
///
/// This enumeration corresponds to the `SYNC_STATUS` PostgreSQL enum and is used
/// to track the current state of connection synchronization.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::SyncStatus"]
pub enum SyncStatus {
    /// Sync is pending
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Sync is in progress
    #[db_rename = "running"]
    #[serde(rename = "running")]
    Running,

    /// Sync was cancelled
    #[db_rename = "cancelled"]
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl SyncStatus {
    /// Returns whether the sync is pending.
    #[inline]
    pub fn is_pending(self) -> bool {
        matches!(self, SyncStatus::Pending)
    }

    /// Returns whether the sync is running.
    #[inline]
    pub fn is_running(self) -> bool {
        matches!(self, SyncStatus::Running)
    }

    /// Returns whether the sync was cancelled.
    #[inline]
    pub fn is_cancelled(self) -> bool {
        matches!(self, SyncStatus::Cancelled)
    }

    /// Returns whether the sync is in progress (pending or running).
    #[inline]
    pub fn is_in_progress(self) -> bool {
        matches!(self, SyncStatus::Pending | SyncStatus::Running)
    }
}
