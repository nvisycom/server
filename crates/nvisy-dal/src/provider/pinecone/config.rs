//! Pinecone configuration types.

use serde::{Deserialize, Serialize};

/// Pinecone credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PineconeCredentials {
    /// Pinecone API key.
    pub api_key: String,
}

/// Pinecone parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PineconeParams {
    /// Index name.
    pub index: String,
    /// Namespace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}
