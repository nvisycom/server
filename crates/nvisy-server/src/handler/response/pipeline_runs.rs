//! Pipeline run response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspacePipelineRun as PipelineRunModel;
use nvisy_postgres::query::RunFiles;
use nvisy_postgres::types::{Handle, PipelineRunStatus, PipelineTriggerType, RunId, RunMetadata};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

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
    /// Source document this run analyzes / redacts.
    pub input_file_id: Uuid,
    /// Display name of the source document, for showing the run without a
    /// separate file lookup. `None` if the file was removed (e.g. by retention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
    /// Redacted document produced by the run, once it completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_id: Option<Uuid>,
    /// Display name of the redacted output, once the run completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_name: Option<String>,
    /// Account that triggered the run.
    pub triggered_by: AccountRef,
    /// How the run was triggered.
    pub trigger_type: PipelineTriggerType,
    /// Current run status.
    ///
    /// The detections are available to fetch from the run's `detections`
    /// endpoint once this reaches `analyzed`.
    pub status: PipelineRunStatus,
    /// Human-readable failure reason, present only when the run `failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Non-encrypted metadata for filtering/display.
    pub metadata: RunMetadata,
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
    /// owning pipeline and workspace, the triggering account, and the resolved
    /// input/output file display names.
    pub fn from_model(
        run: PipelineRunModel,
        pipeline_slug: Handle,
        workspace_slug: Handle,
        triggered_by: AccountRef,
        files: RunFiles,
    ) -> Self {
        // Surface the failure reason (written to metadata.error by the worker /
        // enqueue-failure path) as a dedicated field for a failed run.
        let metadata = run.metadata.or_default();
        let error = metadata.error.clone();

        Self {
            id: RunId::from_uuid(run.id),
            pipeline_slug,
            workspace_slug,
            input_file_id: run.input_file_id,
            input_file_name: files.input,
            output_file_id: run.output_file_id,
            output_file_name: files.output,
            triggered_by,
            trigger_type: run.trigger_type,
            status: run.status,
            error,
            metadata,
            started_at: run.started_at.into(),
            completed_at: run.completed_at.map(Into::into),
        }
    }
}
