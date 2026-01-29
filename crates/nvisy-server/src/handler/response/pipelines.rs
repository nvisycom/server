//! Pipeline response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::PipelineStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Artifact, Page};

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
    pub definition: serde_json::Value,
    /// Artifacts produced by pipeline runs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl Pipeline {
    /// Creates a new instance of [`Pipeline`] from the database model.
    pub fn from_model(pipeline: model::WorkspacePipeline) -> Self {
        Self {
            pipeline_id: pipeline.id,
            workspace_id: pipeline.workspace_id,
            account_id: pipeline.account_id,
            name: pipeline.name,
            description: pipeline.description,
            status: pipeline.status,
            definition: pipeline.definition,
            artifacts: Vec::new(),
            created_at: pipeline.created_at.into(),
            updated_at: pipeline.updated_at.into(),
        }
    }

    /// Creates a pipeline response with artifacts.
    pub fn from_model_with_artifacts(
        pipeline: model::WorkspacePipeline,
        artifacts: Vec<model::WorkspacePipelineArtifact>,
    ) -> Self {
        Self {
            pipeline_id: pipeline.id,
            workspace_id: pipeline.workspace_id,
            account_id: pipeline.account_id,
            name: pipeline.name,
            description: pipeline.description,
            status: pipeline.status,
            definition: pipeline.definition,
            artifacts: artifacts.into_iter().map(Artifact::from_model).collect(),
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
    pub fn from_model(pipeline: model::WorkspacePipeline) -> Self {
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
