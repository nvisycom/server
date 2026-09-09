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
#[must_use]
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
#[must_use]
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
    /// Start timestamp override, for tests only.
    #[cfg(any(feature = "test_util", test))]
    pub started_at: Option<Timestamp>,
}

impl NewWorkspaceConnectionSync {
    /// A minimal sync for `connection_id`, attributed to `account_id`, for tests.
    /// The trigger, status, records, and attempt take their database defaults
    /// (`on_demand`, `running`, `0`, `1`), so the sync starts in an active state.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(connection_id: Uuid, account_id: Uuid) -> Self {
        Self {
            connection_id,
            account_id,
            ..Default::default()
        }
    }
}

/// Data for updating a workspace connection sync.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_connection_syncs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
