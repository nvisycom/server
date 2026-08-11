//! Pipeline response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{Handle, PipelineStatus, RetentionOverride};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AccountRef, Page};
use crate::handler::request::PipelineDefinition;

/// Pipeline response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pipeline {
    /// URL slug of the pipeline, unique within its workspace.
    pub slug: Handle,
    /// Handle of the workspace this pipeline belongs to.
    pub workspace_slug: Handle,
    /// Account that created this pipeline.
    pub created_by: AccountRef,
    /// Pipeline display name.
    pub display_name: String,
    /// Pipeline description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Pipeline lifecycle status.
    pub status: PipelineStatus,
    /// Detection + redaction configuration.
    pub definition: PipelineDefinition,
    /// Per-scope data-retention override, when the pipeline sets one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<RetentionOverride>,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl Pipeline {
    /// Creates a response from the database model and its reference slugs.
    ///
    /// The `policy_slugs` come from the join table and are merged with the stored
    /// engine config to rebuild the full definition. Fails if the stored config
    /// JSON does not decode to the current schema.
    pub fn from_model(
        pipeline: model::WorkspacePipeline,
        workspace_slug: Handle,
        created_by: AccountRef,
        policy_slugs: Vec<Handle>,
    ) -> serde_json::Result<Self> {
        let retention = RetentionOverride::from_pipeline_metadata(&pipeline.metadata);
        let definition = PipelineDefinition::from_parts(pipeline.definition, policy_slugs)?;
        Ok(Self {
            slug: pipeline.slug,
            workspace_slug,
            created_by,
            display_name: pipeline.display_name,
            description: pipeline.description,
            status: pipeline.status,
            definition,
            retention,
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
    pub slug: Handle,
    /// Handle of the workspace this pipeline belongs to.
    pub workspace_slug: Handle,
    /// Account that created this pipeline.
    pub created_by: AccountRef,
    /// Pipeline display name.
    pub display_name: String,
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
    /// Creates a new instance of [`PipelineSummary`] from the database model and
    /// its creator.
    pub fn from_model(
        pipeline: model::WorkspacePipeline,
        workspace_slug: Handle,
        created_by: AccountRef,
    ) -> Self {
        Self {
            slug: pipeline.slug,
            workspace_slug,
            created_by,
            display_name: pipeline.display_name,
            description: pipeline.description,
            status: pipeline.status,
            created_at: pipeline.created_at.into(),
            updated_at: pipeline.updated_at.into(),
        }
    }
}

/// Paginated list of pipeline summaries.
pub type PipelineSummariesPage = Page<PipelineSummary>;
