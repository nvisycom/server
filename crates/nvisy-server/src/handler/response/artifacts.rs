//! Pipeline artifact response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspacePipelineArtifact;
use nvisy_postgres::types::ArtifactType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response type for a pipeline artifact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// Unique artifact identifier.
    pub id: Uuid,
    /// Pipeline run that produced this artifact.
    pub run_id: Uuid,
    /// File storing the artifact data.
    pub file_id: Uuid,
    /// Type of artifact (input, output, intermediate).
    pub artifact_type: ArtifactType,
    /// Extended metadata (checksums, counts, etc.).
    pub metadata: serde_json::Value,
    /// When the artifact was created.
    pub created_at: Timestamp,
}

impl Artifact {
    /// Creates an artifact response from the database model.
    pub fn from_model(artifact: WorkspacePipelineArtifact) -> Self {
        Self {
            id: artifact.id,
            run_id: artifact.run_id,
            file_id: artifact.file_id,
            artifact_type: artifact.artifact_type,
            metadata: artifact.metadata,
            created_at: artifact.created_at.into(),
        }
    }
}
