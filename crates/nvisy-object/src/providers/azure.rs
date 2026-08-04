//! Azure Blob Storage provider using [`object_store::azure::MicrosoftAzureBuilder`].

use std::fmt;

use derive_more::Deref;
use object_store::azure::MicrosoftAzureBuilder;
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
            builder = builder.with_sas_authorization(parse_sas(sas));
        }

        if let Some(endpoint) = &creds.endpoint {
            builder = builder.with_endpoint(endpoint.clone());
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), Self::ID))?;

        Ok(Self(ObjectStoreClient::new(store)))
    }
}

/// Parses a SAS token query string into key/value pairs.
///
/// Accepts an optional leading `?`, splits on `&`, and treats a pair with no
/// `=` as a key with an empty value. Empty segments (e.g. a trailing `&`) are
/// skipped.
fn parse_sas(sas: &str) -> Vec<(String, String)> {
    sas.trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (key.to_string(), value.to_string()),
            None => (pair.to_string(), String::new()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_sas;

    #[test]
    fn parses_leading_question_mark_and_pairs() {
        let pairs = parse_sas("?sv=2021&sig=ab%2Fcd");
        assert_eq!(
            pairs,
            vec![
                ("sv".to_string(), "2021".to_string()),
                ("sig".to_string(), "ab%2Fcd".to_string()),
            ]
        );
    }

    #[test]
    fn keeps_equals_in_value_and_skips_empty_segments() {
        let pairs = parse_sas("a=b=c&&flag");
        assert_eq!(
            pairs,
            vec![
                ("a".to_string(), "b=c".to_string()),
                ("flag".to_string(), String::new()),
            ]
        );
    }
}
