//! Pipeline run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspacePipelineRun as PipelineRunModel;
use nvisy_postgres::types::{Handle, PipelineRunStatus, PipelineTriggerType, RunId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for a pipeline run.
///
/// A run is addressed by its own opaque id; the owning pipeline and workspace
/// slugs are carried for context.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRun {
    /// Opaque identifier of the run.
    pub id: RunId,
    /// Handle of the pipeline this run belongs to.
    pub pipeline_slug: Handle,
    /// Handle of the workspace this run belongs to.
    pub workspace_slug: Handle,
    /// File this run analyzes / redacts.
    pub file_id: Uuid,
    /// Handle of the account that triggered the run, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_username: Option<Handle>,
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
        pipeline_slug: Handle,
        workspace_slug: Handle,
        trigger_username: Option<Handle>,
    ) -> Self {
        Self {
            id: RunId::from_uuid(run.id),
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
