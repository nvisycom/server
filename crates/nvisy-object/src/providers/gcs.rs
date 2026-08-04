//! Google Cloud Storage provider using [`object_store::gcp::GoogleCloudStorageBuilder`].

use std::fmt;

use derive_more::Deref;
use object_store::gcp::GoogleCloudStorageBuilder;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::Deserialize;

use super::{Client, redact};
use crate::client::ObjectStoreClient;
use crate::error::Error;

/// Typed credentials for Google Cloud Storage.
///
/// Secret fields are masked in the [`Debug`] output; the struct is
/// deserialize-only and never serialized back out.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct GcsCredentials {
    /// GCS bucket name.
    pub bucket: String,
    /// Path to a service account key JSON file on the local filesystem.
    #[serde(default)]
    pub service_account_path: Option<String>,
    /// Inline service account key JSON (the file contents, not a path).
    #[serde(default)]
    pub service_account_key_json: Option<String>,
    /// Custom endpoint URL (for testing with a fake GCS server).
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl fmt::Debug for GcsCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GcsCredentials")
            .field("bucket", &self.bucket)
            .field("service_account_path", &self.service_account_path)
            .field(
                "service_account_key_json",
                &redact(self.service_account_key_json.as_deref()),
            )
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

/// Google Cloud Storage-backed object storage client.
#[derive(Deref)]
pub struct GcsProvider(ObjectStoreClient);

impl Client for GcsProvider {
    type Credentials = GcsCredentials;

    const ID: &str = "gcs";

    async fn connect(creds: &Self::Credentials) -> Result<Self, Error> {
        let mut builder = GoogleCloudStorageBuilder::new().with_bucket_name(&creds.bucket);

        if let Some(path) = &creds.service_account_path {
            builder = builder.with_service_account_path(path);
        }

        if let Some(key_json) = &creds.service_account_key_json {
            builder = builder.with_service_account_key(key_json);
        }

        if let Some(endpoint) = &creds.endpoint {
            builder = builder.with_url(endpoint);
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), Self::ID))?;

        Ok(Self(ObjectStoreClient::new(store)))
    }
}
