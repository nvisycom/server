//! Pipeline model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::pipelines;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt, PipelineStatus};

/// Pipeline model representing a workflow definition in the system.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Pipeline {
    /// Unique pipeline identifier.
    pub id: Uuid,
    /// Reference to the workspace this pipeline belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that created this pipeline.
    pub account_id: Uuid,
    /// Pipeline name.
    pub name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline lifecycle status.
    pub status: PipelineStatus,
    /// Pipeline definition (steps, input/output schemas, etc.).
    pub definition: serde_json::Value,
    /// Extended metadata.
    pub metadata: serde_json::Value,
    /// Timestamp when the pipeline was created.
    pub created_at: Timestamp,
    /// Timestamp when the pipeline was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the pipeline was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new pipeline.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewPipeline {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// Pipeline name.
    pub name: String,
    /// Pipeline description.
    pub description: Option<String>,
    /// Pipeline status.
    pub status: Option<PipelineStatus>,
    /// Pipeline definition.
    pub definition: Option<serde_json::Value>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a pipeline.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = pipelines)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdatePipeline {
    /// Pipeline name.
    pub name: Option<String>,
    /// Pipeline description.
    pub description: Option<Option<String>>,
    /// Pipeline status.
    pub status: Option<PipelineStatus>,
    /// Pipeline definition.
    pub definition: Option<serde_json::Value>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl Pipeline {
    /// Returns whether the pipeline is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the pipeline is in draft status.
    pub fn is_draft(&self) -> bool {
        self.status.is_draft()
    }

    /// Returns whether the pipeline is enabled.
    pub fn is_enabled(&self) -> bool {
        self.status.is_enabled()
    }

    /// Returns whether the pipeline is disabled.
    pub fn is_disabled(&self) -> bool {
        self.status.is_disabled()
    }

    /// Returns whether the pipeline has a description.
    pub fn has_description(&self) -> bool {
        self.description.as_ref().is_some_and(|d| !d.is_empty())
    }
}

impl HasCreatedAt for Pipeline {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for Pipeline {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for Pipeline {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
