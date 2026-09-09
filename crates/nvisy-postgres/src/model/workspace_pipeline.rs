//! Workspace pipeline model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_pipelines;
use crate::types::{Handle, Json, PipelineMetadata, PipelineStatus};

/// Workspace pipeline model representing a workflow definition in the system.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
#[must_use]
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

impl NewWorkspacePipeline {
    /// A minimal draft pipeline for `workspace_id`, for tests.
    ///
    /// The slug is unique per call (so several test pipelines fit in one
    /// workspace), the definition is a non-empty placeholder object, and the
    /// status takes its `draft` database default.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid) -> Self {
        Self {
            workspace_id,
            account_id,
            slug: Handle::test(),
            display_name: "Test Pipeline".to_owned(),
            description: None,
            status: None,
            definition: Some(serde_json::json!({ "recognizers": [] })),
            metadata: None,
        }
    }
}

/// Data for updating a workspace pipeline.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
