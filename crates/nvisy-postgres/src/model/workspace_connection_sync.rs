//! Workspace connection sync model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_connection_syncs;
use crate::types::{SyncStatus, SyncTriggerType};

/// A connection sync: one synchronization execution of a connection.
///
/// Each sync records how it was triggered, how many records it processed, and
/// its outcome. Syncs are incremental: each lists the source and imports only
/// objects not already imported, so re-running picks up new objects without any
/// stored cursor. The connection's current sync state is derived from its most
/// recent sync rather than stored on the connection.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_connection_syncs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceConnectionSync {
    /// Unique sync identifier.
    pub id: Uuid,
    /// Connection the sync synchronizes.
    pub connection_id: Uuid,
    /// Account the sync is attributed to (the user who started it, or the
    /// connection's creator for a scheduled sync).
    pub account_id: Uuid,
    /// How the sync was initiated.
    pub trigger_type: SyncTriggerType,
    /// Current sync status.
    pub status: SyncStatus,
    /// Number of records processed.
    pub records_synced: i64,
    /// 1-based attempt number; scheduled syncs may be retried up to a limit.
    pub attempt: i32,
    /// Failure detail when status is failed.
    pub error_message: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: JsonValue,
    /// When the sync started.
    pub started_at: Timestamp,
    /// When the sync finished.
    pub completed_at: Option<Timestamp>,
}

/// Data for creating a new workspace connection sync.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_connection_syncs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceConnectionSync {
    /// Connection ID (required).
    pub connection_id: Uuid,
    /// Account the sync is attributed to (required).
    pub account_id: Uuid,
    /// Trigger type.
    pub trigger_type: Option<SyncTriggerType>,
    /// Initial status.
    pub status: Option<SyncStatus>,
    /// Number of records processed.
    pub records_synced: Option<i64>,
    /// 1-based attempt number (defaults to 1).
    pub attempt: Option<i32>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
}

/// Data for updating a workspace connection sync.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_connection_syncs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceConnectionSync {
    /// Sync status.
    pub status: Option<SyncStatus>,
    /// Number of records processed.
    pub records_synced: Option<i64>,
    /// Failure detail when status is failed.
    pub error_message: Option<Option<String>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// When the sync finished.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspaceConnectionSync {
    /// Returns whether the sync is in progress (pending or running).
    pub fn is_in_progress(&self) -> bool {
        self.status.is_in_progress()
    }

    /// Returns whether the sync reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    /// Returns whether the sync finished successfully.
    pub fn is_completed(&self) -> bool {
        self.status.is_completed()
    }

    /// Returns whether the sync failed.
    pub fn is_failed(&self) -> bool {
        self.status.is_failed()
    }
}
