//! Milvus configuration types.

use serde::{Deserialize, Serialize};

/// Default Milvus port.
fn default_port() -> u16 {
    19530
}

/// Milvus credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilvusCredentials {
    /// Milvus server host.
    pub host: String,
    /// Milvus server port.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Username for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Password for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Milvus parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilvusParams {
    /// Collection name.
    pub collection: String,
    /// Database name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}
