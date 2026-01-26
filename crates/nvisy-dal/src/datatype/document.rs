//! Document data type for JSON documents.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{DataType, Metadata};

/// A document with flexible JSON content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier.
    pub id: String,
    /// Document content as JSON.
    pub content: Value,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: Metadata,
}

impl DataType for Document {}
