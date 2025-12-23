//! Project pipeline response types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a project pipeline.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPipeline {
    /// Unique pipeline identifier.
    pub pipeline_id: Uuid,
    /// ID of the project this pipeline belongs to.
    pub project_id: Uuid,
    /// Pipeline name.
    pub name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline configuration as JSON.
    pub configuration: serde_json::Value,
    /// Whether the pipeline is enabled.
    pub enabled: bool,
    /// Pipeline triggers configuration.
    pub triggers: Option<serde_json::Value>,
    /// Current status of the pipeline.
    pub status: String,
    /// Last execution timestamp.
    pub last_execution: Option<OffsetDateTime>,
    /// Next scheduled execution timestamp.
    pub next_execution: Option<OffsetDateTime>,
    /// Number of successful executions.
    pub success_count: i64,
    /// Number of failed executions.
    pub failure_count: i64,
    /// Timestamp when the pipeline was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the pipeline was soft-deleted.
    pub deleted_at: Option<OffsetDateTime>,
}

/// Response for listing project pipelines.
pub type ProjectPipelines = Vec<ProjectPipeline>;
