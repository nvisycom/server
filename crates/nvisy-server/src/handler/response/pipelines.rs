//! Pipeline response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{PipelineStatus, WorkspaceSlug};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Artifact, Page};
use crate::handler::request::PipelineDefinition;

/// Pipeline response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pipeline {
    /// Unique pipeline identifier.
    pub pipeline_id: Uuid,
    /// Slug of the workspace this pipeline belongs to.
    pub workspace_slug: WorkspaceSlug,
    /// Account that created this pipeline.
    pub account_id: Uuid,
    /// Pipeline name.
    pub name: String,
    /// Pipeline description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Pipeline lifecycle status.
    pub status: PipelineStatus,
    /// Detection + redaction configuration.
    pub definition: PipelineDefinition,
    /// Artifacts produced by pipeline runs.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl Pipeline {
    /// Creates a response from the database model and its reference ids.
    ///
    /// The `policy_ids` / `context_ids` come from the join tables and are merged
    /// with the stored engine config to rebuild the full definition. Fails if the
    /// stored config JSON does not decode to the current schema.
    pub fn from_model(
        pipeline: model::WorkspacePipeline,
        workspace_slug: WorkspaceSlug,
        policy_ids: Vec<Uuid>,
        context_ids: Vec<Uuid>,
    ) -> serde_json::Result<Self> {
        Self::assemble(
            pipeline,
            workspace_slug,
            Vec::new(),
            policy_ids,
            context_ids,
        )
    }

    /// Creates a pipeline response with artifacts and reference ids.
    pub fn from_model_with_artifacts(
        pipeline: model::WorkspacePipeline,
        workspace_slug: WorkspaceSlug,
        artifacts: Vec<model::WorkspacePipelineArtifact>,
        policy_ids: Vec<Uuid>,
        context_ids: Vec<Uuid>,
    ) -> serde_json::Result<Self> {
        let artifacts = artifacts.into_iter().map(Artifact::from_model).collect();
        Self::assemble(pipeline, workspace_slug, artifacts, policy_ids, context_ids)
    }

    /// Shared assembly: decodes the stored config and merges the references.
    fn assemble(
        pipeline: model::WorkspacePipeline,
        workspace_slug: WorkspaceSlug,
        artifacts: Vec<Artifact>,
        policy_ids: Vec<Uuid>,
        context_ids: Vec<Uuid>,
    ) -> serde_json::Result<Self> {
        let definition =
            PipelineDefinition::from_parts(pipeline.definition, policy_ids, context_ids)?;
        Ok(Self {
            pipeline_id: pipeline.id,
            workspace_slug,
            account_id: pipeline.account_id,
            name: pipeline.name,
            description: pipeline.description,
            status: pipeline.status,
            definition,
            artifacts,
            created_at: pipeline.created_at.into(),
            updated_at: pipeline.updated_at.into(),
        })
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
