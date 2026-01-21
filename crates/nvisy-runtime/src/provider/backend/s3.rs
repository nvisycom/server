//! Amazon S3 provider.

use nvisy_dal::provider::S3Config;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Amazon S3 credentials.
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

/// Amazon S3 parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct S3Params {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Bucket name.
    pub bucket: String,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl S3Params {
    /// Combines params with credentials to create a full provider config.
    pub fn into_config(self, credentials: S3Credentials) -> S3Config {
        let mut config = S3Config::new(self.bucket, credentials.region)
            .with_credentials(credentials.access_key_id, credentials.secret_access_key);

        if let Some(endpoint) = credentials.endpoint {
            config = config.with_endpoint(endpoint);
        }
        if let Some(prefix) = self.prefix {
            config = config.with_prefix(prefix);
        }

        config
    }
}
