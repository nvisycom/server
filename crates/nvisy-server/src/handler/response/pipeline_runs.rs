//! Pipeline run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspacePipelineRun as PipelineRunModel;
use nvisy_postgres::types::{PipelineRunStatus, PipelineTriggerType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRun {
    /// Unique run identifier.
    pub id: Uuid,
    /// Pipeline this run belongs to.
    pub pipeline_id: Uuid,
    /// Account that triggered the run (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,
    /// How the run was triggered.
    pub trigger_type: PipelineTriggerType,
    /// Current execution status.
    pub status: PipelineRunStatus,
    /// Pipeline definition snapshot at run time.
    pub definition_snapshot: serde_json::Value,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: serde_json::Value,
    /// Execution logs as JSON array.
    pub logs: serde_json::Value,
    /// When execution started.
    pub started_at: Timestamp,
    /// When execution completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
}

/// Paginated response for pipeline runs.
pub type PipelineRunsPage = Page<PipelineRun>;

impl PipelineRun {
    /// Creates a pipeline run response from the database model.
    pub fn from_model(run: PipelineRunModel) -> Self {
        Self {
            id: run.id,
            pipeline_id: run.pipeline_id,
            account_id: run.account_id,
            trigger_type: run.trigger_type,
            status: run.status,
            definition_snapshot: run.definition_snapshot,
            metadata: run.metadata,
            logs: run.logs,
            started_at: run.started_at.into(),
            completed_at: run.completed_at.map(Into::into),
        }
    }
}
