//! Workspace connection run model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::workspace_connection_runs;
use crate::types::{SyncStatus, SyncTriggerType};

/// A connection sync run: one synchronization execution of a connection.
///
/// Each run records how it was triggered, how many records it processed, and
/// its outcome. Runs are incremental: each lists the source and imports only
/// objects not already imported, so re-running a sync picks up new objects
/// without any stored cursor. The connection's current sync state is derived
/// from its most recent run rather than stored on the connection.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_connection_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceConnectionRun {
    /// Unique sync run identifier.
    pub id: Uuid,
    /// Connection the run synchronizes.
    pub connection_id: Uuid,
    /// Account the run is attributed to (the user who started it, or the
    /// connection's creator for a scheduled run).
    pub account_id: Uuid,
    /// How the run was initiated.
    pub trigger_type: SyncTriggerType,
    /// Current run status.
    pub status: SyncStatus,
    /// Number of records processed.
    pub records_synced: i64,
    /// 1-based attempt number; scheduled runs may be retried up to a limit.
    pub attempt: i32,
    /// Failure detail when status is failed.
    pub error_message: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: JsonValue,
    /// When the run started.
    pub started_at: Timestamp,
    /// When the run finished.
    pub completed_at: Option<Timestamp>,
}

/// Data for creating a new workspace connection run.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_connection_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceConnectionRun {
    /// Connection ID (required).
    pub connection_id: Uuid,
    /// Account the run is attributed to (required).
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

/// Data for updating a workspace connection run.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_connection_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceConnectionRun {
    /// Run status.
    pub status: Option<SyncStatus>,
    /// Number of records processed.
    pub records_synced: Option<i64>,
    /// Failure detail when status is failed.
    pub error_message: Option<Option<String>>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: Option<JsonValue>,
    /// When the run finished.
    pub completed_at: Option<Option<Timestamp>>,
}

impl WorkspaceConnectionRun {
    /// Returns whether the run is in progress (pending or running).
    pub fn is_in_progress(&self) -> bool {
        self.status.is_in_progress()
    }

    /// Returns whether the run reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    /// Returns whether the run finished successfully.
    pub fn is_completed(&self) -> bool {
        self.status.is_completed()
    }

    /// Returns whether the run failed.
    pub fn is_failed(&self) -> bool {
        self.status.is_failed()
    }
}
