//! Qdrant configuration types.

use serde::{Deserialize, Serialize};

/// Qdrant credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantCredentials {
    /// Qdrant server URL (e.g., "http://localhost:6334").
    pub url: String,
    /// API key for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Qdrant parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantParams {
    /// Collection name.
    pub collection: String,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}
