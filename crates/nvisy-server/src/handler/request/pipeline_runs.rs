//! Pipeline run request types (detect).

use nvisy_schema::plan::ScopeParams;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

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
