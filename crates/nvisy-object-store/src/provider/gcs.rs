//! Google Cloud Storage provider using [`object_store::gcp::GoogleCloudStorageBuilder`].

use nvisy_core::net::EndpointPolicy;
use nvisy_core::{Error, Secret};
use object_store::ClientOptions;
use object_store::gcp::GoogleCloudStorageBuilder;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::ObjectStoreClient;

/// Typed credentials for Google Cloud Storage.
///
/// Secret fields are masked in the [`Debug`] output. Serialization exists only
/// to persist the credentials encrypted at rest; they are never returned in API
/// responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct GcsCredentials {
    /// GCS bucket name.
    pub bucket: String,
    /// Service account key JSON (the key file's contents). Required: without
    /// it the builder would fall back to the server's own application-default
    /// or instance-metadata credentials.
    pub service_account_key: Secret<String>,
    /// Custom endpoint URL (for testing with a fake GCS server).
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl GcsCredentials {
    /// Connects to the store, validating any custom endpoint under `policy`.
    pub(super) async fn connect(&self, policy: EndpointPolicy) -> Result<ObjectStoreClient, Error> {
        let mut builder = GoogleCloudStorageBuilder::new()
            .with_bucket_name(&self.bucket)
            .with_service_account_key(self.service_account_key.expose());

        if let Some(endpoint) = &self.endpoint {
            // Validate the custom endpoint: only http(s), and plaintext http only
            // for a local emulator (e.g. a fake-gcs server).
            let allow_http = super::endpoint_allow_http(policy, endpoint).await?;
            // `with_base_url` sets a custom service endpoint (e.g. a fake-gcs
            // server); `with_url` instead parses a `gs://bucket/path` object URL,
            // which is not what an endpoint override is.
            builder = builder.with_base_url(endpoint);
            // The GCS builder has no `with_allow_http`; plaintext http for a
            // permitted loopback emulator is enabled through client options.
            if allow_http {
                builder = builder.with_client_options(ClientOptions::new().with_allow_http(true));
            }
        }

        let store = builder
            .build()
            .map_err(|e| Error::invalid(format!("{}: {e}", "gcs")))?;

        Ok(ObjectStoreClient::new(store))
    }
}
