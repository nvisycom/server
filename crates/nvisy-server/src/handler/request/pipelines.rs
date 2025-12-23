//! Project pipeline request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request body for creating a project pipeline.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectPipeline {
    /// Pipeline name.
    #[schema(example = "Build and Deploy")]
    pub name: String,
    /// Pipeline description.
    #[schema(example = "Automated build and deployment pipeline")]
    pub description: Option<String>,
    /// Pipeline configuration as JSON.
    #[schema(example = json!({"stages": ["build", "test", "deploy"]}))]
    pub configuration: serde_json::Value,
    /// Whether the pipeline is enabled.
    #[schema(example = true)]
    pub enabled: Option<bool>,
    /// Pipeline triggers configuration.
    #[schema(example = json!({"on_push": true, "on_pr": false}))]
    pub triggers: Option<serde_json::Value>,
}

/// Request body for updating a project pipeline.
pub type UpdateProjectPipeline = CreateProjectPipeline;

