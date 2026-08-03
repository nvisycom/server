//! Client trait and object storage providers.

mod azure;
mod gcs;
mod s3;

use std::ops::Deref;

pub use azure::{AzureCredentials, AzureProvider};
pub use gcs::{GcsCredentials, GcsProvider};
pub use s3::{S3Credentials, S3Provider};
use serde::de::DeserializeOwned;

use crate::client::ObjectStoreClient;
use crate::error::Error;

/// Authenticated connection to an object storage backend.
///
/// Implementations are newtype wrappers around [`ObjectStoreClient`] that
/// handle credential validation and client construction for a specific
/// provider (e.g. S3, Azure, GCS).
pub trait Client: Deref<Target = ObjectStoreClient> + Send + Sync + 'static {
    /// Strongly-typed credentials for this provider.
    type Credentials: DeserializeOwned + Send;

    /// Unique identifier (e.g. `s3`, `azure`).
    const ID: &str;

    /// Create a connected client from credentials.
    fn connect(creds: &Self::Credentials) -> impl Future<Output = Result<Self, Error>> + Send
    where
        Self: Sized;
}

/// Connects to an object store by runtime provider id, returning the shared
/// [`ObjectStoreClient`] regardless of which provider backs it.
///
/// `raw_credentials` is the provider-specific credential JSON (the shape of
/// [`S3Credentials`], [`AzureCredentials`], or [`GcsCredentials`]). This is the
/// entry point for callers that pick a provider at runtime from stored config
/// rather than at compile time.
///
/// Returns an [`Error`] if `provider_id` is unknown or the credentials do not
/// match the provider's expected shape.
pub async fn connect(
    provider_id: &str,
    raw_credentials: serde_json::Value,
) -> Result<ObjectStoreClient, Error> {
    match provider_id {
        S3Provider::ID => connect_with::<S3Provider>(raw_credentials).await,
        AzureProvider::ID => connect_with::<AzureProvider>(raw_credentials).await,
        GcsProvider::ID => connect_with::<GcsProvider>(raw_credentials).await,
        other => Err(Error::connection(
            format!("unknown object store provider: {other}"),
            "object-store",
        )),
    }
}

/// Deserializes `raw_credentials` into `C::Credentials` and connects, yielding
/// the shared inner client.
async fn connect_with<C: Client>(
    raw_credentials: serde_json::Value,
) -> Result<ObjectStoreClient, Error> {
    let credentials: C::Credentials = serde_json::from_value(raw_credentials)
        .map_err(|e| Error::connection(e.to_string(), C::ID))?;
    let provider = C::connect(&credentials).await?;
    Ok((*provider).clone())
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

    use super::connect;
    use crate::error::ErrorKind;

    #[tokio::test]
    async fn connect_unknown_provider_errors() {
        let err = connect("nope", json!({})).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Connection);
        assert!(err.to_string().contains("unknown object store provider"));
    }

    #[tokio::test]
    async fn connect_s3_builds_from_credentials() {
        // `build()` is lazy (no network), so valid config yields a client.
        let creds = json!({ "bucket": "test-bucket", "region": "us-east-1" });
        assert!(connect("s3", creds).await.is_ok());
    }

    #[tokio::test]
    async fn connect_rejects_malformed_credentials() {
        // Missing the required `bucket` field.
        let err = connect("s3", json!({ "region": "us-east-1" }))
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Connection);
    }
}
