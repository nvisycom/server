//! Amazon S3 configuration types.

use serde::{Deserialize, Serialize};

/// Amazon S3 credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Credentials {
    /// AWS region.
    pub region: String,
    /// Access key ID.
    pub access_key_id: String,
    /// Secret access key.
    pub secret_access_key: String,
    /// Custom endpoint URL (for S3-compatible storage like MinIO, R2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Amazon S3 parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct S3Params {
    /// Bucket name.
    pub bucket: String,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}
