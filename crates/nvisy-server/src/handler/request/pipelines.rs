//! Project pipeline request types.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Request body for creating a project pipeline.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePipeline {
    /// Pipeline name.
    pub name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline configuration as JSON.
    pub configuration: serde_json::Value,
    /// Whether the pipeline is enabled.
    pub enabled: Option<bool>,
    /// Pipeline triggers configuration.
    pub triggers: Option<serde_json::Value>,
}

/// Request body for updating a project pipeline.
pub type UpdatePipeline = CreatePipeline;

