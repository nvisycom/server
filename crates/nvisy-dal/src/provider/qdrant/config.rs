//! Qdrant configuration.

use serde::{Deserialize, Serialize};

/// Qdrant configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantConfig {
    /// Qdrant server URL (e.g., "http://localhost:6334").
    pub url: String,
    /// API key for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Default collection name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl QdrantConfig {
    /// Creates a new Qdrant configuration.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            api_key: None,
            collection: None,
            dimensions: None,
        }
    }

    /// Sets the API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets the default collection.
    pub fn with_collection(mut self, collection: impl Into<String>) -> Self {
        self.collection = Some(collection.into());
        self
    }

    /// Sets the vector dimensions.
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}
