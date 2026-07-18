//! Connection sync run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceConnectionRun as ConnectionRunModel;
use nvisy_postgres::types::{Slug, SyncStatus, SyncTriggerType, Username};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Page;

/// Response type for a connection sync run.
///
/// A run is addressed as `(connection slug, run number)`, so it carries no
/// surrogate id of its own.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRun {
    /// Sequential run number within the connection (the run's identity).
    pub run_number: i32,
    /// Slug of the connection this run belongs to.
    pub connection_slug: Slug,
    /// Slug of the workspace this run belongs to.
    pub workspace_slug: Slug,
    /// Handle of the account that triggered the run, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_username: Option<Username>,
    /// How the run was triggered.
    pub trigger_type: SyncTriggerType,
    /// Current run status.
    pub status: SyncStatus,
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
    /// Creates a connection run response from the database model, the slugs of
    /// its owning connection and workspace, and the triggering account's handle.
    pub fn from_model(
        run: ConnectionRunModel,
        connection_slug: Slug,
        workspace_slug: Slug,
        trigger_username: Option<Username>,
    ) -> Self {
        Self {
            run_number: run.run_number,
            connection_slug,
            workspace_slug,
            trigger_username,
            trigger_type: run.trigger_type,
            status: run.status,
            records_synced: run.records_synced,
            error_message: run.error_message,
            metadata: run.metadata,
            started_at: run.started_at.into(),
            completed_at: run.completed_at.map(Into::into),
        }
    }
}
