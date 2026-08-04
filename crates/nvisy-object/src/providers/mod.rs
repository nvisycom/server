//! Client trait and object storage providers.

mod azure;
mod gcs;
mod s3;

use std::ops::Deref;

pub use azure::{AzureCredentials, AzureProvider};
pub use gcs::{GcsCredentials, GcsProvider};
use object_store::prefix::PrefixStore;
pub use s3::{S3Credentials, S3Provider};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::ObjectStoreClient;
use crate::error::Error;

/// A fully-typed object-store connection configuration.
///
/// The `provider` tag selects the variant and thereby the credential shape, so
/// an S3 connection cannot carry Azure credentials. Each variant carries a
/// shared optional `root_path` and a nested `credentials` object, giving a wire
/// shape like
/// `{ "provider": "s3", "rootPath": "in/", "credentials": { "bucket": "b", "accessKeyId": "..." } }`.
///
/// Secrets are masked in [`Debug`]; serialization exists only to persist the
/// config encrypted at rest, never to return it in API responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(
    tag = "provider",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConnectionConfig {
    /// S3-compatible provider (AWS S3, MinIO, and so on).
    S3 {
        /// Optional root prefix within the bucket; keys resolve relative to it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root_path: Option<String>,
        /// S3 credentials.
        credentials: S3Credentials,
    },
    /// Azure Blob Storage provider.
    Azure {
        /// Optional root prefix within the container; keys resolve relative to it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root_path: Option<String>,
        /// Azure credentials.
        credentials: AzureCredentials,
    },
    /// Google Cloud Storage provider.
    Gcs {
        /// Optional root prefix within the bucket; keys resolve relative to it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root_path: Option<String>,
        /// GCS credentials.
        credentials: GcsCredentials,
    },
}

impl ConnectionConfig {
    /// The provider identifier for this config (`s3`, `azure`, `gcs`), used for
    /// the stored `provider` column and for filtering.
    #[must_use]
    pub fn provider_id(&self) -> &'static str {
        match self {
            Self::S3 { .. } => S3Provider::ID,
            Self::Azure { .. } => AzureProvider::ID,
            Self::Gcs { .. } => GcsProvider::ID,
        }
    }

    /// The configured root prefix, if any.
    #[must_use]
    pub fn root_path(&self) -> Option<&str> {
        match self {
            Self::S3 { root_path, .. }
            | Self::Azure { root_path, .. }
            | Self::Gcs { root_path, .. } => root_path.as_deref(),
        }
    }
}

/// Scopes a client's keys under `root_path` when one is set, so callers address
/// objects relative to it.
fn with_root_path(client: ObjectStoreClient, root_path: Option<&str>) -> ObjectStoreClient {
    match root_path.map(str::trim).filter(|p| !p.is_empty()) {
        Some(prefix) => ObjectStoreClient::new(PrefixStore::new(client.0, prefix)),
        None => client,
    }
}

/// Authenticated connection to an object storage backend.
///
/// Implementations are newtype wrappers around [`ObjectStoreClient`] that
/// handle credential validation and client construction for a specific
/// provider (e.g. S3, Azure, GCS).
pub trait Client: Deref<Target = ObjectStoreClient> + Send + Sync + 'static {
    /// Strongly-typed credentials for this provider.
    type Credentials: Send;

    /// Unique identifier (e.g. `s3`, `azure`).
    const ID: &str;

    /// Create a connected client from credentials.
    fn connect(creds: &Self::Credentials) -> impl Future<Output = Result<Self, Error>> + Send
    where
        Self: Sized;
}

/// Connects to an object store from a typed [`ConnectionConfig`], returning the
/// shared [`ObjectStoreClient`] regardless of which provider backs it.
///
/// The config's variant selects the provider, so there is no runtime provider
/// string to validate; the client is scoped under the config's root path.
pub async fn connect(config: &ConnectionConfig) -> Result<ObjectStoreClient, Error> {
    let client = match config {
        ConnectionConfig::S3 { credentials, .. } => connect_client::<S3Provider>(credentials).await,
        ConnectionConfig::Azure { credentials, .. } => {
            connect_client::<AzureProvider>(credentials).await
        }
        ConnectionConfig::Gcs { credentials, .. } => {
            connect_client::<GcsProvider>(credentials).await
        }
    }?;
    Ok(with_root_path(client, config.root_path()))
}

/// Connects a specific provider and unwraps it to the shared client type.
async fn connect_client<C: Client>(
    credentials: &C::Credentials,
) -> Result<ObjectStoreClient, Error> {
    Ok((*C::connect(credentials).await?).clone())
}

/// Renders a secret field for [`Debug`]: `<set>` when present, `<unset>` when
/// absent. Never reveals the value.
fn redact(value: Option<&str>) -> &'static str {
    match value {
        Some(_) => "<set>",
        None => "<unset>",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ConnectionConfig, connect};

    #[tokio::test]
    async fn deserializes_provider_tagged_config() {
        let config: ConnectionConfig = serde_json::from_value(json!({
            "provider": "s3",
            "credentials": { "bucket": "test-bucket", "region": "us-east-1" },
        }))
        .unwrap();
        assert_eq!(config.provider_id(), "s3");
        assert!(matches!(config, ConnectionConfig::S3 { .. }));
    }

    #[tokio::test]
    async fn rejects_unknown_provider_tag() {
        let result: Result<ConnectionConfig, _> =
            serde_json::from_value(json!({ "provider": "nope", "credentials": { "bucket": "b" } }));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_missing_required_field_for_provider() {
        // S3 without the required `bucket` field.
        let result: Result<ConnectionConfig, _> = serde_json::from_value(
            json!({ "provider": "s3", "credentials": { "region": "us-east-1" } }),
        );
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connect_s3_builds_from_typed_config() {
        // `build()` is lazy (no network), so valid config yields a client.
        let config = ConnectionConfig::S3 {
            root_path: None,
            credentials: serde_json::from_value(
                json!({ "bucket": "test-bucket", "region": "us-east-1" }),
            )
            .unwrap(),
        };
        assert!(connect(&config).await.is_ok());
    }

    #[tokio::test]
    async fn parses_root_path() {
        let config: ConnectionConfig = serde_json::from_value(json!({
            "provider": "s3",
            "rootPath": "incoming/documents",
            "credentials": { "bucket": "test-bucket", "region": "us-east-1" },
        }))
        .unwrap();
        assert_eq!(config.root_path(), Some("incoming/documents"));
    }
}
