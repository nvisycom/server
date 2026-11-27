//! S3-compatible provider using [`object_store::aws::AmazonS3Builder`].
//!
//! Works with AWS S3, `MinIO`, and any S3-compatible service.

use std::fmt;

use derive_more::Deref;
use nvisy_core::net::EndpointPolicy;
use object_store::aws::AmazonS3Builder;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Client, REDACTED, redact};
use crate::client::ObjectStoreClient;
use crate::error::Error;

/// Typed credentials for S3-compatible provider.
///
/// Secret fields are masked in the [`Debug`] output. Serialization exists only
/// to persist the credentials encrypted at rest; they are never returned in API
/// responses.
#[derive(Clone, Deserialize, Serialize)]
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
    pub access_key_id: String,
    /// Secret access key paired with `access_key_id`.
    pub secret_access_key: String,
    /// Session token for temporary credentials.
    #[serde(default)]
    pub session_token: Option<String>,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

impl fmt::Debug for S3Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Credentials")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("endpoint", &self.endpoint)
            .field("access_key_id", &REDACTED)
            .field("secret_access_key", &REDACTED)
            .field("session_token", &redact(self.session_token.as_deref()))
            .finish()
    }
}

/// S3-backed object storage client.
#[derive(Deref)]
pub struct S3Provider(ObjectStoreClient);

impl Client for S3Provider {
    type Credentials = S3Credentials;

    const ID: &str = "s3";

    async fn connect(creds: &Self::Credentials, policy: EndpointPolicy) -> Result<Self, Error> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&creds.bucket)
            .with_region(&creds.region)
            .with_access_key_id(&creds.access_key_id)
            .with_secret_access_key(&creds.secret_access_key);

        if let Some(endpoint) = &creds.endpoint {
            // Validate the custom endpoint and enable plaintext HTTP only for a
            // local emulator (e.g. MinIO), never for a remote host.
            let allow_http = super::endpoint_allow_http(policy, endpoint, Self::ID).await?;
            builder = builder.with_endpoint(endpoint);
            if allow_http {
                builder = builder.with_allow_http(true);
            }
        }

        if let Some(token) = &creds.session_token {
            builder = builder.with_token(token);
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), Self::ID))?;

        Ok(Self(ObjectStoreClient::new(store)))
    }
}
