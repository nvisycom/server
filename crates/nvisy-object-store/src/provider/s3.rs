//! S3-compatible provider using [`object_store::aws::AmazonS3Builder`].
//!
//! Works with AWS S3, `MinIO`, and any S3-compatible service.

use nvisy_core::net::EndpointPolicy;
use nvisy_core::{Error, Secret};
use object_store::aws::AmazonS3Builder;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::ObjectStoreClient;

/// Typed credentials for S3-compatible provider.
///
/// Secret fields are masked in the [`Debug`] output. Serialization exists only
/// to persist the credentials encrypted at rest; they are never returned in API
/// responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct S3Credentials {
    /// S3 bucket name.
    pub bucket: String,
    /// AWS region (defaults to `us-east-1`).
    #[serde(default = "default_region")]
    pub region: String,
    /// Endpoint URL (e.g. `http://localhost:9000` for `MinIO`).
    /// Required for non-AWS S3-compatible services.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Access key ID. Required: without static credentials the builder would
    /// fall back to the server's own ambient identity (instance metadata).
    pub access_key_id: Secret<String>,
    /// Secret access key paired with `access_key_id`.
    pub secret_access_key: Secret<String>,
    /// Session token for temporary credentials.
    #[serde(default)]
    pub session_token: Option<Secret<String>>,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

impl S3Credentials {
    /// Connects to the store, validating any custom endpoint under `policy`.
    pub(super) async fn connect(&self, policy: EndpointPolicy) -> Result<ObjectStoreClient, Error> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&self.bucket)
            .with_region(&self.region)
            .with_access_key_id(self.access_key_id.expose())
            .with_secret_access_key(self.secret_access_key.expose());

        if let Some(endpoint) = &self.endpoint {
            // Validate the custom endpoint and enable plaintext HTTP only for a
            // local emulator (e.g. MinIO), never for a remote host.
            let allow_http = super::endpoint_allow_http(policy, endpoint).await?;
            builder = builder.with_endpoint(endpoint);
            if allow_http {
                builder = builder.with_allow_http(true);
            }
        }

        if let Some(token) = &self.session_token {
            builder = builder.with_token(token.expose());
        }

        let store = builder
            .build()
            .map_err(|e| Error::invalid(format!("{}: {e}", "s3")))?;

        Ok(ObjectStoreClient::new(store))
    }
}
