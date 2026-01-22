//! Pipeline response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::PipelineStatus;
use nvisy_runtime::definition::Workflow;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Pipeline response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pipeline {
    /// Unique pipeline identifier.
    pub pipeline_id: Uuid,
    /// Workspace this pipeline belongs to.
    pub workspace_id: Uuid,
    /// Account that created this pipeline.
    pub account_id: Uuid,
    /// Pipeline name.
    pub name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline lifecycle status.
    pub status: PipelineStatus,
    /// Pipeline definition (workflow graph).
    #[schemars(with = "serde_json::Value")]
    pub definition: Workflow,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl Pipeline {
    /// Creates a new instance of [`Pipeline`] from the database model.
    pub fn from_model(pipeline: model::Pipeline) -> Self {
        let definition: Workflow =
            serde_json::from_value(pipeline.definition).unwrap_or_default();
        Self {
            pipeline_id: pipeline.id,
            workspace_id: pipeline.workspace_id,
            account_id: pipeline.account_id,
            name: pipeline.name,
            description: pipeline.description,
            status: pipeline.status,
            definition,
            created_at: pipeline.created_at.into(),
            updated_at: pipeline.updated_at.into(),
        }
    }
}

/// Paginated list of pipelines.
pub type PipelinesPage = Page<Pipeline>;

/// Summary response for pipeline (used in lists).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineSummary {
    /// Unique pipeline identifier.
    pub pipeline_id: Uuid,
    /// Pipeline name.
    pub name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline lifecycle status.
    pub status: PipelineStatus,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl PipelineSummary {
    /// Creates a new instance of [`PipelineSummary`] from the database model.
    pub fn from_model(pipeline: model::Pipeline) -> Self {
        Self {
            pipeline_id: pipeline.id,
            name: pipeline.name,
            description: pipeline.description,
            status: pipeline.status,
            created_at: pipeline.created_at.into(),
            updated_at: pipeline.updated_at.into(),
        }
    }
}

/// Paginated list of pipeline summaries.
pub type PipelineSummariesPage = Page<PipelineSummary>;
