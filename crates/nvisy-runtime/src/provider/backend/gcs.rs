//! Google Cloud Storage provider.

use nvisy_dal::provider::GcsConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::WorkflowResult;

/// Google Cloud Storage credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsCredentials {
    /// Service account credentials JSON.
    pub credentials_json: String,
}

/// Google Cloud Storage parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcsParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Bucket name.
    pub bucket: String,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl IntoProvider for GcsParams {
    type Credentials = GcsCredentials;
    type Output = GcsConfig;

    fn into_provider(self, credentials: Self::Credentials) -> WorkflowResult<Self::Output> {
        let mut config = GcsConfig::new(self.bucket).with_credentials(credentials.credentials_json);

        if let Some(prefix) = self.prefix {
            config = config.with_prefix(prefix);
        }

        Ok(config)
    }
}
