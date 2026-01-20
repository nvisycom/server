//! Document data type for JSON documents.

use serde::{Deserialize, Serialize};

use super::{DataType, Metadata};

/// A document with flexible JSON content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier.
    pub id: String,
    /// Document content as JSON.
    pub content: serde_json::Value,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

impl Document {
    /// Creates a new document.
    pub fn new(id: impl Into<String>, content: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            content,
            metadata: Metadata::new(),
        }
    }

    /// Sets metadata.
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl DataType for Document {
    const TYPE_ID: &'static str = "document";

    fn data_type_id() -> super::DataTypeId {
        super::DataTypeId::Document
    }
}
