//! Pipeline response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::PipelineStatus;
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
    /// Pipeline definition (steps, configuration).
    pub definition: serde_json::Value,
    /// Extended metadata.
    pub metadata: serde_json::Value,
    /// Number of steps in the pipeline.
    pub step_count: usize,
    /// Whether the pipeline can be executed.
    pub is_runnable: bool,
    /// Whether the pipeline can be edited.
    pub is_editable: bool,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl Pipeline {
    /// Creates a new instance of [`Pipeline`] from the database model.
    pub fn from_model(pipeline: model::Pipeline) -> Self {
        Self {
            pipeline_id: pipeline.id,
            workspace_id: pipeline.workspace_id,
            account_id: pipeline.account_id,
            name: pipeline.name.clone(),
            description: pipeline.description.clone(),
            status: pipeline.status,
            step_count: pipeline.step_count(),
            is_runnable: pipeline.is_runnable(),
            is_editable: pipeline.is_editable(),
            definition: pipeline.definition.clone(),
            metadata: pipeline.metadata.clone(),
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
    /// Number of steps in the pipeline.
    pub step_count: usize,
    /// Whether the pipeline can be executed.
    pub is_runnable: bool,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl PipelineSummary {
    /// Creates a new instance of [`PipelineSummary`] from the database model.
    pub fn from_model(pipeline: model::Pipeline) -> Self {
        let step_count = pipeline.step_count();
        let is_runnable = pipeline.is_runnable();
        Self {
            pipeline_id: pipeline.id,
            name: pipeline.name,
            description: pipeline.description,
            status: pipeline.status,
            step_count,
            is_runnable,
            created_at: pipeline.created_at.into(),
            updated_at: pipeline.updated_at.into(),
        }
    }
}

/// Paginated list of pipeline summaries.
pub type PipelineSummariesPage = Page<PipelineSummary>;
