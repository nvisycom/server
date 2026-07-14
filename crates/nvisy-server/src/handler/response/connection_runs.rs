//! Connection sync run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceConnectionRun as ConnectionRunModel;
use nvisy_postgres::types::{SyncStatus, SyncTriggerType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for a connection sync run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Connection this run synchronizes.
    pub connection_id: Uuid,
    /// Account that triggered the run (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    /// How the run was triggered.
    pub trigger_type: SyncTriggerType,
    /// Current run status.
    pub status: SyncStatus,
    /// Sequence number within the connection (display only).
    pub run_number: i32,
    /// Number of records processed by the run.
    pub records_synced: i64,
    /// Failure detail when the run failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: serde_json::Value,
    /// When the run started.
    pub started_at: Timestamp,
    /// When the run finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
}

/// Paginated response for connection sync runs.
pub type ConnectionRunsPage = Page<ConnectionRun>;

impl ConnectionRun {
    /// Creates a connection run response from the database model.
    pub fn from_model(run: ConnectionRunModel) -> Self {
        Self {
            id: run.id,
            connection_id: run.connection_id,
            account_id: run.account_id,
            trigger_type: run.trigger_type,
            status: run.status,
            run_number: run.run_number,
            records_synced: run.records_synced,
            error_message: run.error_message,
            metadata: run.metadata,
            started_at: run.started_at.into(),
            completed_at: run.completed_at.map(Into::into),
        }
    }
}
