//! Google Cloud Storage provider using [`object_store::gcp::GoogleCloudStorageBuilder`].

use derive_more::Deref;
use object_store::gcp::GoogleCloudStorageBuilder;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Client;
use crate::client::ObjectStoreClient;
use crate::types::Error;

/// Typed credentials for Google Cloud Storage.
#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct GcsCredentials {
    /// GCS bucket name.
    pub bucket: String,
    /// Path to a JSON service account key file.
    #[serde(default)]
    pub service_account_key: Option<String>,
    /// Custom endpoint URL (for testing with a fake GCS server).
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// Google Cloud Storage-backed object storage client.
#[derive(Deref)]
pub struct GcsProvider(ObjectStoreClient);

impl Client for GcsProvider {
    type Credentials = GcsCredentials;

    const ID: &str = "gcs";

    async fn connect(creds: &Self::Credentials) -> Result<Self, Error> {
        let mut builder = GoogleCloudStorageBuilder::new().with_bucket_name(&creds.bucket);

        if let Some(key_path) = &creds.service_account_key {
            builder = builder.with_service_account_key(key_path);
        }

        if let Some(endpoint) = &creds.endpoint {
            builder = builder.with_url(endpoint);
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), Self::ID, true))?;

        Ok(Self(ObjectStoreClient::new(store)))
    }
}
