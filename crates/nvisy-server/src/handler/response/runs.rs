//! Integration run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceIntegrationRun;
use nvisy_postgres::types::{IntegrationStatus, RunType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for an integration run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Workspace ID.
    pub workspace_id: Uuid,
    /// Integration ID (if associated with an integration).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration_id: Option<Uuid>,
    /// Account that triggered the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    /// Run type.
    pub run_type: RunType,
    /// Current status.
    pub status: IntegrationStatus,
    /// Run metadata, results, and error details.
    pub metadata: serde_json::Value,
    /// When the run started.
    pub started_at: Timestamp,
    /// When the run completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
}

/// Paginated response for integration runs.
pub type IntegrationRunsPage = Page<IntegrationRun>;

impl IntegrationRun {
    pub fn from_model(run: WorkspaceIntegrationRun) -> Self {
        Self {
            id: run.id,
            workspace_id: run.workspace_id,
            integration_id: run.integration_id,
            account_id: run.account_id,
            run_type: run.run_type,
            status: run.run_status,
            metadata: run.metadata,
            started_at: run.started_at.into(),
            completed_at: run.completed_at.map(Into::into),
        }
    }
}
