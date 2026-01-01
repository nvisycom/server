//! Integration run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceIntegrationRun;
use nvisy_postgres::types::IntegrationStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    /// Run name.
    pub run_name: String,
    /// Run type.
    pub run_type: String,
    /// Current status.
    pub status: IntegrationStatus,
    /// Run metadata, results, and error details.
    pub metadata: serde_json::Value,
    /// When the run started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<Timestamp>,
    /// When the run completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
    /// When the run was created.
    pub created_at: Timestamp,
}

/// List of integration runs.
pub type IntegrationRuns = Vec<IntegrationRun>;

impl IntegrationRun {
    /// Creates an IntegrationRun response from a database model.
    pub fn from_model(run: WorkspaceIntegrationRun) -> Self {
        Self {
            id: run.id,
            workspace_id: run.workspace_id,
            integration_id: run.integration_id,
            account_id: run.account_id,
            run_name: run.run_name,
            run_type: run.run_type,
            status: run.run_status,
            metadata: run.metadata,
            started_at: run.started_at.map(Into::into),
            completed_at: run.completed_at.map(Into::into),
            created_at: run.created_at.into(),
        }
    }

    /// Creates a list of IntegrationRun responses from database models.
    pub fn from_models(models: Vec<WorkspaceIntegrationRun>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }
}
