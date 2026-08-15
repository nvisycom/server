//! Pipeline run request types (detect).

use elide_pipeline::plan::ScopeParams;
use nvisy_postgres::types::{PipelineRunStatus, PipelineTriggerType, RunFilter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Query parameters for listing runs across a workspace.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRunsQuery {
    /// Filter by run status.
    pub status: Option<PipelineRunStatus>,
    /// Filter by the source file the run analyzes.
    pub file_id: Option<Uuid>,
    /// Filter by the owning pipeline.
    pub pipeline_id: Option<Uuid>,
    /// Filter by the account that triggered the run.
    pub triggered_by: Option<Uuid>,
    /// Filter by how the run was initiated (user vs system).
    pub trigger_type: Option<PipelineTriggerType>,
}

impl From<WorkspaceRunsQuery> for RunFilter {
    fn from(query: WorkspaceRunsQuery) -> Self {
        RunFilter {
            status: query.status,
            input_file_id: query.file_id,
            pipeline_id: query.pipeline_id,
            account_id: query.triggered_by,
            trigger_type: query.trigger_type,
        }
    }
}

/// Query parameters for listing a single pipeline's runs.
///
/// The pipeline is fixed by the route, so it narrows only by status, file,
/// trigger account, and trigger type.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunsQuery {
    /// Filter by run status.
    pub status: Option<PipelineRunStatus>,
    /// Filter by the source file the run analyzes.
    pub file_id: Option<Uuid>,
    /// Filter by the account that triggered the run.
    pub triggered_by: Option<Uuid>,
    /// Filter by how the run was initiated (user vs system).
    pub trigger_type: Option<PipelineTriggerType>,
}

impl From<PipelineRunsQuery> for RunFilter {
    fn from(query: PipelineRunsQuery) -> Self {
        RunFilter {
            status: query.status,
            input_file_id: query.file_id,
            pipeline_id: None,
            account_id: query.triggered_by,
            trigger_type: query.trigger_type,
        }
    }
}

/// Request payload to start a run (detect) over a file.
///
/// Analyzes the file with the pipeline's configuration and returns the run,
/// which holds the findings for review before redaction.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreatePipelineRun {
    /// The file to analyze.
    pub file_id: Uuid,
    /// Per-document scope (languages, jurisdictions, document labels).
    ///
    /// Overrides the pipeline's `defaultScope` when present; absent falls back to
    /// the pipeline default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeParams>,
}
