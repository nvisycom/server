//! Workspace connection sync-schedule model.
//!
//! A connection's scheduled-sync configuration. One row per connection that syncs
//! on a timer. Transfer capability is the connection's `provider_type`, not this
//! row's presence.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_connection_schedule;
use crate::types::{SyncDeletionPolicy, SyncMode};

/// Sync configuration for a sync-capable connection.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_connection_schedule)]
#[diesel(primary_key(connection_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct WorkspaceConnectionSchedule {
    /// The connection this schedule configures.
    pub connection_id: Uuid,
    /// Whether the connection imports data in or exports data out.
    pub sync_mode: SyncMode,
    /// Cron expression for scheduled imports; `None` means manual-only.
    pub schedule_cron: Option<String>,
    /// How an import reconciles files whose source object was deleted.
    pub deletion_policy: SyncDeletionPolicy,
}

/// Data for creating a connection's sync schedule.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_connection_schedule)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceConnectionSchedule {
    /// The connection this schedule configures.
    pub connection_id: Uuid,
    /// Sync direction (defaults to import).
    pub sync_mode: Option<SyncMode>,
    /// Cron expression for scheduled imports.
    pub schedule_cron: Option<String>,
    /// Deletion reconciliation policy (defaults to ignore).
    pub deletion_policy: Option<SyncDeletionPolicy>,
}

impl NewWorkspaceConnectionSchedule {
    /// A minimal manual-only schedule for `connection_id`, for tests. The mode
    /// and deletion policy take their database defaults (`import`, `ignore`) and
    /// there is no cron, so the connection does not sync on a timer.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(connection_id: Uuid) -> Self {
        Self {
            connection_id,
            sync_mode: None,
            schedule_cron: None,
            deletion_policy: None,
        }
    }
}
