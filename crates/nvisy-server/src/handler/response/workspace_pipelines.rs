//! `WorkspacePipeline` response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{Handle, PipelineStatus, RetentionOverride};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};
use crate::handler::request::PipelineDefinition;

/// `WorkspacePipeline` response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePipeline {
    /// Unique identifier of the pipeline.
    pub id: Uuid,
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// URL-safe workspace handle. Display-only.
    pub workspace_handle: Handle,
    /// Account that created this pipeline.
    pub created_by: AccountRef,
    /// `WorkspacePipeline` display name.
    pub display_name: String,
    /// `WorkspacePipeline` description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `WorkspacePipeline` lifecycle status.
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

impl WorkspacePipeline {
    /// Creates a response from the database model and its reference ids.
    ///
    /// The `policy_ids` come from the join table and are merged with the stored
    /// engine config to rebuild the full definition. Fails if the stored config
    /// JSON does not decode to the current schema.
    pub fn from_model(
        pipeline: model::WorkspacePipeline,
        workspace_id: Uuid,
        workspace_handle: Handle,
        created_by: AccountRef,
        policy_ids: Vec<Uuid>,
    ) -> serde_json::Result<Self> {
        let retention = pipeline.metadata.or_default().retention;
        let definition = PipelineDefinition::from_parts(pipeline.definition, policy_ids)?;
        Ok(Self {
            id: pipeline.id,
            workspace_id,
            workspace_handle,
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
pub type WorkspacePipelinesPage = Page<WorkspacePipeline>;

/// Summary response for pipeline (used in lists).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePipelineSummary {
    /// Unique identifier of the pipeline.
    pub id: Uuid,
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// URL-safe workspace handle. Display-only.
    pub workspace_handle: Handle,
    /// Account that created this pipeline.
    pub created_by: AccountRef,
    /// `WorkspacePipeline` display name.
    pub display_name: String,
    /// `WorkspacePipeline` description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `WorkspacePipeline` lifecycle status.
    pub status: PipelineStatus,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
}

impl WorkspacePipelineSummary {
    /// Creates a new instance of [`WorkspacePipelineSummary`] from the database model and
    /// its creator.
    pub fn from_model(
        pipeline: model::WorkspacePipeline,
        workspace_id: Uuid,
        workspace_handle: Handle,
        created_by: AccountRef,
    ) -> Self {
        Self {
            id: pipeline.id,
            workspace_id,
            workspace_handle,
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
pub type WorkspacePipelineSummariesPage = Page<WorkspacePipelineSummary>;
