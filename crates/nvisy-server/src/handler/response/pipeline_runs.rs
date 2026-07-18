//! Pipeline run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspacePipelineRun as PipelineRunModel;
use nvisy_postgres::types::{PipelineRunStatus, PipelineTriggerType, Slug, Username};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for a pipeline run.
///
/// A run is addressed as `(pipeline slug, run number)`, so it carries no
/// surrogate id of its own.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRun {
    /// Sequential run number within the pipeline (the run's identity).
    pub run_number: i32,
    /// Slug of the pipeline this run belongs to.
    pub pipeline_slug: Slug,
    /// Slug of the workspace this run belongs to.
    pub workspace_slug: Slug,
    /// File this run analyzes / redacts.
    pub file_id: Uuid,
    /// Handle of the account that triggered the run, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_username: Option<Username>,
    /// How the run was triggered.
    pub trigger_type: PipelineTriggerType,
    /// Current run status.
    ///
    /// The detections are available to fetch from the run's `detections`
    /// endpoint once this reaches `analyzed`.
    pub status: PipelineRunStatus,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: serde_json::Value,
    /// When the run started.
    pub started_at: Timestamp,
    /// When the run completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
}

/// Paginated response for pipeline runs.
pub type PipelineRunsPage = Page<PipelineRun>;

impl PipelineRun {
    /// Creates a pipeline run response from the database model, the slugs of its
    /// owning pipeline and workspace, and the triggering account's handle.
    pub fn from_model(
        run: PipelineRunModel,
        pipeline_slug: Slug,
        workspace_slug: Slug,
        trigger_username: Option<Username>,
    ) -> Self {
        Self {
            run_number: run.run_number,
            pipeline_slug,
            workspace_slug,
            file_id: run.file_id,
            trigger_username,
            trigger_type: run.trigger_type,
            status: run.status,
            metadata: run.metadata,
            started_at: run.started_at.into(),
            completed_at: run.completed_at.map(Into::into),
        }
    }
}
