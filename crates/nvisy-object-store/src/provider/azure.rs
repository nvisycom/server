//! Azure Blob Storage provider using [`object_store::azure::MicrosoftAzureBuilder`].

use derive_more::Deref;
use nvisy_core::net::EndpointPolicy;
use nvisy_core::{Error, Secret};
use object_store::azure::{MicrosoftAzureBuilder, split_sas};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Client;
use crate::client::ObjectStoreClient;

/// Typed credentials for Azure Blob Storage.
///
/// Secret fields are masked in the [`Debug`] output. Serialization exists only
/// to persist the credentials encrypted at rest; they are never returned in API
/// responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct AzureCredentials {
    /// Azure storage container name.
    pub container: String,
    /// Azure storage account name.
    pub account_name: String,
    /// How to authenticate. Required: without it the builder would fall back to
    /// the server's own managed identity.
    pub auth: AzureAuth,
    /// Custom endpoint URL (for Azure Stack or Azurite).
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// Azure Blob Storage authentication, tagged by `type`: `{ "type": "accessKey",
/// "accessKey": "..." }` or `{ "type": "sasToken", "sasToken": "..." }`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AzureAuth {
    /// A storage account access key.
    AccessKey {
        /// The account key.
        access_key: Secret<String>,
    },
    /// A Shared Access Signature token.
    SasToken {
        /// The SAS token (query string form).
        sas_token: Secret<String>,
    },
}

/// Azure Blob Storage-backed object storage client.
#[derive(Deref)]
pub struct AzureProvider(ObjectStoreClient);

impl Client for AzureProvider {
    type Credentials = AzureCredentials;

    const ID: &str = "azure";

    async fn connect(creds: &Self::Credentials, policy: EndpointPolicy) -> Result<Self, Error> {
        let mut builder = MicrosoftAzureBuilder::new()
            .with_container_name(&creds.container)
            .with_account(&creds.account_name);

        builder = match &creds.auth {
            AzureAuth::AccessKey { access_key } => builder.with_access_key(access_key.expose()),
            AzureAuth::SasToken { sas_token } => {
                // `split_sas` percent-decodes and validates the token, handling
                // the edge cases (e.g. `=` inside a value) a naive split would
                // corrupt.
                let pairs = split_sas(sas_token.expose())
                    .map_err(|e| Error::invalid(format!("{}: {e}", Self::ID)))?;
                builder.with_sas_authorization(pairs)
            }
        };

        if let Some(endpoint) = &creds.endpoint {
            // Validate the custom endpoint under the deployment policy; enable
            // plaintext HTTP only for a local emulator (Azurite).
            let allow_http = super::endpoint_allow_http(policy, endpoint).await?;
            builder = builder.with_endpoint(endpoint.clone());
            if allow_http {
                builder = builder.with_allow_http(true);
            }
        }

        let store = builder
            .build()
            .map_err(|e| Error::invalid(format!("{}: {e}", Self::ID)))?;

        Ok(Self(ObjectStoreClient::new(store)))
    }
}
