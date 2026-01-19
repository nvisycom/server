//! Amazon S3 configuration.

use serde::{Deserialize, Serialize};

/// Amazon S3 configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct S3Config {
    /// Bucket name.
    pub bucket: String,
    /// AWS region.
    pub region: String,
    /// Custom endpoint URL (for S3-compatible storage like MinIO, R2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Access key ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
    /// Secret access key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl S3Config {
    /// Creates a new S3 configuration.
    pub fn new(bucket: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            region: region.into(),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            prefix: None,
        }
    }

    /// Sets the custom endpoint (for S3-compatible storage).
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Sets the access credentials.
    pub fn with_credentials(
        mut self,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key_id.into());
        self.secret_access_key = Some(secret_access_key.into());
        self
    }

    /// Sets the path prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }
}
