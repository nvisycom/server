//! Common types used across integrations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata associated with data or vectors.
pub type Metadata = HashMap<String, serde_json::Value>;

/// A vector with its ID and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorData {
    /// Unique identifier for this vector.
    pub id: String,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Optional metadata associated with this vector.
    #[serde(default)]
    pub metadata: Metadata,
}

impl VectorData {
    /// Creates a new vector data with the given ID and vector.
    pub fn new(id: impl Into<String>, vector: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            vector,
            metadata: Metadata::new(),
        }
    }

    /// Adds metadata to this vector.
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Result from a vector similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// The ID of the matched vector.
    pub id: String,
    /// Similarity score (interpretation depends on distance metric).
    pub score: f32,
    /// The vector data, if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
    /// Metadata associated with this vector.
    #[serde(default)]
    pub metadata: Metadata,
}
