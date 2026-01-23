//! Google Cloud Storage configuration types.

use serde::{Deserialize, Serialize};

/// Google Cloud Storage credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsCredentials {
    /// Service account credentials JSON.
    pub credentials_json: String,
}

/// Google Cloud Storage parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcsParams {
    /// Bucket name.
    pub bucket: String,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}
