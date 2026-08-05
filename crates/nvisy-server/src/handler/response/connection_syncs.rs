//! Connection sync response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceConnectionRun;
use nvisy_postgres::types::{ConnectionId, SyncStatus, SyncTriggerType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Creator, Page};

/// A connection sync run (import or export).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSync {
    /// Unique sync identifier.
    pub id: Uuid,
    /// The connection this sync belongs to.
    pub connection_id: ConnectionId,
    /// Account that triggered the sync.
    pub trigger: Creator,
    /// How the sync was triggered.
    pub trigger_type: SyncTriggerType,
    /// Current status of the sync.
    pub status: SyncStatus,
    /// Number of objects transferred so far.
    pub records_synced: i64,
    /// 1-based attempt number; scheduled syncs may be retried on failure.
    pub attempt: i32,
    /// Failure reason when the sync failed; omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// When the sync started.
    pub started_at: Timestamp,
    /// When the sync finished, if it has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
}

impl ConnectionSync {
    /// Builds a response from the run model and the triggering account.
    pub fn from_model(run: WorkspaceConnectionRun, trigger: Creator) -> Self {
        Self {
            id: run.id,
            connection_id: ConnectionId::from_uuid(run.connection_id),
            trigger,
            trigger_type: run.trigger_type,
            status: run.status,
            records_synced: run.records_synced,
            attempt: run.attempt,
            error_message: run.error_message,
            started_at: run.started_at.into(),
            completed_at: run.completed_at.map(Into::into),
        }
    }
}

/// Paginated list of connection syncs.
pub type ConnectionSyncsPage = Page<ConnectionSync>;
