//! Azure Blob Storage provider using [`object_store::azure::MicrosoftAzureBuilder`].

use std::fmt;

use derive_more::Deref;
use object_store::azure::{MicrosoftAzureBuilder, split_sas};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Client, redact};
use crate::client::ObjectStoreClient;
use crate::error::Error;

/// Typed credentials for Azure Blob Storage.
///
/// Secret fields are masked in the [`Debug`] output. Serialization exists only
/// to persist the credentials encrypted at rest; they are never returned in API
/// responses.
#[derive(Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct AzureCredentials {
    /// Azure storage container name.
    pub container: String,
    /// Azure storage account name.
    pub account_name: String,
    /// Storage account access key.
    #[serde(default)]
    pub access_key: Option<String>,
    /// Shared Access Signature token.
    #[serde(default)]
    pub sas_token: Option<String>,
    /// Custom endpoint URL (for Azure Stack or Azurite).
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl fmt::Debug for AzureCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureCredentials")
            .field("container", &self.container)
            .field("account_name", &self.account_name)
            .field("access_key", &redact(self.access_key.as_deref()))
            .field("sas_token", &redact(self.sas_token.as_deref()))
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

/// Azure Blob Storage-backed object storage client.
#[derive(Deref)]
pub struct AzureProvider(ObjectStoreClient);

impl Client for AzureProvider {
    type Credentials = AzureCredentials;

    const ID: &str = "azure";

    async fn connect(creds: &Self::Credentials) -> Result<Self, Error> {
        let mut builder = MicrosoftAzureBuilder::new()
            .with_container_name(&creds.container)
            .with_account(&creds.account_name);

        if let Some(key) = &creds.access_key {
            builder = builder.with_access_key(key);
        }

        if let Some(sas) = &creds.sas_token {
            // `split_sas` percent-decodes and validates the token, handling the
            // edge cases (e.g. `=` inside a value) a naive split would corrupt.
            let pairs = split_sas(sas).map_err(|e| Error::connection(e.to_string(), Self::ID))?;
            builder = builder.with_sas_authorization(pairs);
        }

        if let Some(endpoint) = &creds.endpoint {
            // Validate the custom endpoint and enable plaintext HTTP only for a
            // local emulator (Azurite), never for a remote host.
            let allow_http = super::endpoint_allow_http(endpoint, Self::ID)?;
            builder = builder.with_endpoint(endpoint.clone());
            if allow_http {
                builder = builder.with_allow_http(true);
            }
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), Self::ID))?;

        Ok(Self(ObjectStoreClient::new(store)))
    }
}
