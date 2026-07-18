//! Pipeline response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{PipelineStatus, Slug, Username};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Artifact, Page};
use crate::handler::request::PipelineDefinition;

/// Pipeline response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pipeline {
    /// URL slug of the pipeline, unique within its workspace.
    pub slug: Slug,
    /// Slug of the workspace this pipeline belongs to.
    pub workspace_slug: Slug,
    /// Handle of the account that created this pipeline.
    pub creator_username: Username,
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
    /// Creates a response from the database model and its reference slugs.
    ///
    /// The `policy_slugs` / `context_slugs` come from the join tables and are
    /// merged with the stored engine config to rebuild the full definition.
    /// Fails if the stored config JSON does not decode to the current schema.
    pub fn from_model(
        pipeline: model::WorkspacePipeline,
        workspace_slug: Slug,
        creator_username: Username,
        policy_slugs: Vec<Slug>,
        context_slugs: Vec<Slug>,
    ) -> serde_json::Result<Self> {
        Self::assemble(
            pipeline,
            workspace_slug,
            creator_username,
            Vec::new(),
            policy_slugs,
            context_slugs,
        )
    }

    /// Creates a pipeline response with artifacts and reference slugs.
    pub fn from_model_with_artifacts(
        pipeline: model::WorkspacePipeline,
        workspace_slug: Slug,
        creator_username: Username,
        artifacts: Vec<model::WorkspacePipelineArtifact>,
        policy_slugs: Vec<Slug>,
        context_slugs: Vec<Slug>,
    ) -> serde_json::Result<Self> {
        let artifacts = artifacts.into_iter().map(Artifact::from_model).collect();
        Self::assemble(
            pipeline,
            workspace_slug,
            creator_username,
            artifacts,
            policy_slugs,
            context_slugs,
        )
    }

    /// Shared assembly: decodes the stored config and merges the references.
    fn assemble(
        pipeline: model::WorkspacePipeline,
        workspace_slug: Slug,
        creator_username: Username,
        artifacts: Vec<Artifact>,
        policy_slugs: Vec<Slug>,
        context_slugs: Vec<Slug>,
    ) -> serde_json::Result<Self> {
        let definition =
            PipelineDefinition::from_parts(pipeline.definition, policy_slugs, context_slugs)?;
        Ok(Self {
            slug: pipeline.slug,
            workspace_slug,
            creator_username,
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
    /// URL slug of the pipeline, unique within its workspace.
    pub slug: Slug,
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
            slug: pipeline.slug,
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
