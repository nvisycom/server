//! Sync status enumeration for connection synchronization operations.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the execution status of a connection sync run.
///
/// This enumeration corresponds to the `SYNC_STATUS` PostgreSQL enum and tracks
/// the state of an individual synchronization run.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::SyncStatus"]
pub enum SyncStatus {
    /// Sync is queued
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Sync is in progress
    #[db_rename = "running"]
    #[serde(rename = "running")]
    Running,

    /// Sync finished successfully
    #[db_rename = "completed"]
    #[serde(rename = "completed")]
    Completed,

    /// Sync failed with error
    #[db_rename = "failed"]
    #[serde(rename = "failed")]
    Failed,

    /// Sync was cancelled
    #[db_rename = "cancelled"]
    #[serde(rename = "cancelled")]
    Cancelled,
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
