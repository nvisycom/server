//! Pinecone configuration.

use serde::{Deserialize, Serialize};

/// Pinecone configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PineconeConfig {
    /// Pinecone API key.
    pub api_key: String,
    /// Environment (e.g., "us-east-1-aws").
    pub environment: String,
    /// Index name.
    pub index: String,
    /// Namespace (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl PineconeConfig {
    /// Creates a new Pinecone configuration.
    pub fn new(
        api_key: impl Into<String>,
        environment: impl Into<String>,
        index: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            environment: environment.into(),
            index: index.into(),
            namespace: None,
            dimensions: None,
        }
    }

    /// Sets the namespace.
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Sets the vector dimensions.
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}
