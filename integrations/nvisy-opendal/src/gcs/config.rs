//! Google Cloud Storage configuration.

use serde::{Deserialize, Serialize};

/// Google Cloud Storage configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcsConfig {
    /// Bucket name.
    pub bucket: String,
    /// Service account credentials JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<String>,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl GcsConfig {
    /// Creates a new GCS configuration.
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            credentials: None,
            prefix: None,
        }
    }

    /// Sets the service account credentials.
    pub fn with_credentials(mut self, credentials: impl Into<String>) -> Self {
        self.credentials = Some(credentials.into());
        self
    }

    /// Sets the path prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }
}
