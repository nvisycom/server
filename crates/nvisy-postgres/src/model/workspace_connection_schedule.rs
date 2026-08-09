//! Workspace connection sync-schedule model.
//!
//! The sync capability's configuration for a connection. One row per
//! sync-capable connection; its presence marks the connection as sync-capable.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_connection_schedule;
use crate::types::{SyncDeletionPolicy, SyncMode};

/// Sync configuration for a sync-capable connection.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Identifiable)]
#[diesel(table_name = workspace_connection_schedule)]
#[diesel(primary_key(connection_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
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

/// Data for updating a connection's sync schedule.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_connection_schedule)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceConnectionSchedule {
    /// Sync direction.
    pub sync_mode: Option<SyncMode>,
    /// Cron expression for scheduled imports (`Some(None)` clears it).
    pub schedule_cron: Option<Option<String>>,
    /// Deletion reconciliation policy.
    pub deletion_policy: Option<SyncDeletionPolicy>,
}
