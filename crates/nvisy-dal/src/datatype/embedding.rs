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

impl DataType for Embedding {}
