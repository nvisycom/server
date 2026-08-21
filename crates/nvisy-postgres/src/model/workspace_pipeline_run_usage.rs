//! Per-model inference usage for a pipeline run.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::workspace_pipeline_run_usage;

/// One model's token usage within a run, as the provider reported it.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = workspace_pipeline_run_usage)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspacePipelineRunUsage {
    /// Unique usage row identifier.
    pub id: Uuid,
    /// The run this usage belongs to.
    pub run_id: Uuid,
    /// The model the recognizers used.
    pub model: String,
    /// The model version, if the provider reported one.
    pub version: Option<String>,
    /// Input/prompt tokens for this model; `None` if not reported.
    pub input_tokens: Option<i64>,
    /// Output/completion tokens for this model; `None` if not reported.
    pub output_tokens: Option<i64>,
    /// Total tokens as reported (not necessarily input + output); `None` if not
    /// reported.
    pub total_tokens: Option<i64>,
    /// Wall-clock time this model spent, in milliseconds.
    pub duration_ms: i64,
}

/// Data for recording one model's usage on a run.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_pipeline_run_usage)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspacePipelineRunUsage {
    /// The run this usage belongs to.
    pub run_id: Uuid,
    /// The model the recognizers used.
    pub model: String,
    /// The model version, if any.
    pub version: Option<String>,
    /// Input/prompt tokens; `None` if not reported.
    pub input_tokens: Option<i64>,
    /// Output/completion tokens; `None` if not reported.
    pub output_tokens: Option<i64>,
    /// Total tokens as reported; `None` if not reported.
    pub total_tokens: Option<i64>,
    /// Wall-clock time this model spent, in milliseconds.
    pub duration_ms: i64,
}
