//! Pipeline run response types.

use jiff::Timestamp;
use nvisy_postgres::model::PipelineRun as PipelineRunModel;
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
    /// Account that triggered the run.
    pub account_id: Uuid,
    /// How the run was triggered.
    pub trigger_type: PipelineTriggerType,
    /// Current execution status.
    pub status: PipelineRunStatus,
    /// Runtime input configuration.
    pub input_config: serde_json::Value,
    /// Runtime output configuration.
    pub output_config: serde_json::Value,
    /// Error details if run failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
    /// Run metrics (duration, resources, etc.).
    pub metrics: serde_json::Value,
    /// When execution started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<Timestamp>,
    /// When execution completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
    /// When run was created/queued.
    pub created_at: Timestamp,
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
            input_config: run.input_config,
            output_config: run.output_config,
            error: run.error,
            metrics: run.metrics,
            started_at: run.started_at.map(Into::into),
            completed_at: run.completed_at.map(Into::into),
            created_at: run.created_at.into(),
        }
    }
}
