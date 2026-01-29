//! Workspace pipeline artifact model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_pipeline_artifacts;
use crate::types::{ArtifactType, HasCreatedAt};

/// Workspace pipeline artifact model representing artifacts produced during pipeline runs.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_pipeline_artifacts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspacePipelineArtifact {
    /// Unique artifact identifier.
    pub id: Uuid,
    /// Reference to the pipeline run.
    pub run_id: Uuid,
    /// Reference to the file storing the artifact data.
    pub file_id: Uuid,
    /// Type of artifact (input, output, intermediate).
    pub artifact_type: ArtifactType,
    /// Extended metadata (checksums, counts, etc.).
    pub metadata: serde_json::Value,
    /// Timestamp when the artifact was created.
    pub created_at: Timestamp,
}

/// Data for creating a new workspace pipeline artifact.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_pipeline_artifacts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspacePipelineArtifact {
    /// Pipeline run ID (required).
    pub run_id: Uuid,
    /// File ID referencing the stored artifact (required).
    pub file_id: Uuid,
    /// Artifact type (required).
    pub artifact_type: ArtifactType,
    /// Metadata (optional).
    pub metadata: Option<serde_json::Value>,
}

impl WorkspacePipelineArtifact {
    /// Returns whether this is an input artifact.
    pub fn is_input(&self) -> bool {
        self.artifact_type.is_input()
    }

    /// Returns whether this is an output artifact.
    pub fn is_output(&self) -> bool {
        self.artifact_type.is_output()
    }

    /// Returns whether this is an intermediate artifact.
    pub fn is_intermediate(&self) -> bool {
        self.artifact_type.is_intermediate()
    }
}

impl HasCreatedAt for WorkspacePipelineArtifact {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}
