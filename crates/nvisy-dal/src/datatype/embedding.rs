//! Embedding data type for vector data.

use serde::{Deserialize, Serialize};

use super::{DataType, Metadata};

/// A vector embedding with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Unique identifier.
    pub id: String,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

impl Embedding {
    /// Creates a new embedding.
    pub fn new(id: impl Into<String>, vector: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            vector,
            metadata: Metadata::new(),
        }
    }

    /// Sets metadata.
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Returns the vector dimensions.
    pub fn dimensions(&self) -> usize {
        self.vector.len()
    }
}

impl DataType for Embedding {
    const TYPE_ID: &'static str = "embedding";

    fn data_type_id() -> super::DataTypeId {
        super::DataTypeId::Embedding
    }
}
