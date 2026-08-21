//! Workspace pipeline model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_pipelines;
use crate::types::{
    Handle, HasCreatedAt, HasDeletedAt, HasUpdatedAt, Json, PipelineMetadata, PipelineStatus,
};

/// Workspace pipeline model representing a workflow definition in the system.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspacePipeline {
    /// Unique pipeline identifier.
    pub id: Uuid,
    /// Reference to the workspace this pipeline belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that created this pipeline.
    pub account_id: Uuid,
    /// URL-safe pipeline identifier, unique within the workspace.
    pub slug: Handle,
    /// Pipeline display name.
    pub display_name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline lifecycle status.
    pub status: PipelineStatus,
    /// Detection/redaction config (nvisy_schema plan as JSON).
    pub definition: serde_json::Value,
    /// Extended metadata.
    pub metadata: Json<PipelineMetadata>,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the pipeline was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace pipeline.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspacePipeline {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// URL-safe pipeline identifier, unique within the workspace.
    pub slug: Handle,
    /// Pipeline display name.
    pub display_name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline status.
    pub status: Option<PipelineStatus>,
    /// Pipeline definition.
    pub definition: Option<serde_json::Value>,
    /// Metadata.
    pub metadata: Option<Json<PipelineMetadata>>,
}

/// Data for updating a workspace pipeline.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspacePipeline {
    /// Pipeline display name.
    pub display_name: Option<String>,
    /// Pipeline description.
    pub description: Option<Option<String>>,
    /// Pipeline status.
    pub status: Option<PipelineStatus>,
    /// Pipeline definition.
    pub definition: Option<serde_json::Value>,
    /// Metadata.
    pub metadata: Option<Json<PipelineMetadata>>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl HasCreatedAt for WorkspacePipeline {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspacePipeline {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for WorkspacePipeline {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
