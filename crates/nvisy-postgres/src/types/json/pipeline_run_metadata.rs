//! Structured metadata for the `workspace_pipeline_runs.metadata` JSONB column.
//!
//! A column the server itself populates has a known shape, so it is typed rather
//! than left as free-form JSON. Read with `Json::or_default` so an absent or
//! older blob yields an empty value rather than failing the read.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Structured metadata for a pipeline run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct RunMetadata {
    /// Free-form labels attached to the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Failure reason recorded when the run failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
