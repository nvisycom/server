//! S3-compatible provider using [`object_store::aws::AmazonS3Builder`].
//!
//! Works with AWS S3, MinIO, and any S3-compatible service.

use derive_more::Deref;
use object_store::aws::AmazonS3Builder;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Client;
use crate::client::ObjectStoreClient;
use crate::types::Error;

/// Typed credentials for S3-compatible provider.
#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct S3Credentials {
    /// S3 bucket name.
    pub bucket: String,
    /// AWS region (defaults to `us-east-1`).
    #[serde(default = "default_region")]
    pub region: String,
    /// Endpoint URL (e.g. `http://localhost:9000` for MinIO).
    /// Required for non-AWS S3-compatible services.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Access key ID for static credentials.
    #[serde(default)]
    pub access_key_id: Option<String>,
    /// Secret access key for static credentials.
    #[serde(default)]
    pub secret_access_key: Option<String>,
    /// Session token for temporary credentials.
    #[serde(default)]
    pub session_token: Option<String>,
}

fn default_region() -> String {
    "us-east-1".to_string()
}

/// S3-backed object storage client.
#[derive(Deref)]
pub struct S3Provider(ObjectStoreClient);

impl Client for S3Provider {
    type Credentials = S3Credentials;

    const ID: &str = "s3";

    async fn connect(creds: &Self::Credentials) -> Result<Self, Error> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&creds.bucket)
            .with_region(&creds.region);

        if let Some(endpoint) = &creds.endpoint {
            builder = builder.with_endpoint(endpoint);
            if endpoint.starts_with("http://") {
                builder = builder.with_allow_http(true);
            }
        }

        if let Some(access_key) = &creds.access_key_id {
            builder = builder.with_access_key_id(access_key);
        }

        if let Some(secret_key) = &creds.secret_access_key {
            builder = builder.with_secret_access_key(secret_key);
        }

        if let Some(token) = &creds.session_token {
            builder = builder.with_token(token);
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), Self::ID, true))?;

        Ok(Self(ObjectStoreClient::new(store)))
    }
}
