//! Client trait for object storage providers.

use std::ops::Deref;

use serde::de::DeserializeOwned;

use crate::client::ObjectStoreClient;
use crate::types::Error;

/// Authenticated connection to an object storage backend.
///
/// Implementations are newtype wrappers around [`ObjectStoreClient`] that
/// handle credential validation and client construction for a specific
/// provider (e.g. S3, Azure, GCS).
pub trait Client: Deref<Target = ObjectStoreClient> + Send + Sync + 'static {
    /// Strongly-typed credentials for this provider.
    type Credentials: DeserializeOwned + Send;

    /// Unique identifier (e.g. "s3", "azure").
    const ID: &str;

    /// Verify that the backing store is reachable.
    fn verify(&self) -> impl Future<Output = Result<(), Error>> + Send {
        self.verify_reachable()
    }

    /// Create a connected client from credentials.
    fn connect(creds: &Self::Credentials) -> impl Future<Output = Result<Self, Error>> + Send
    where
        Self: Sized;

    /// Optional async cleanup when the connection is released.
    ///
    /// The default implementation is a no-op.
    fn disconnect(&self) -> impl Future<Output = ()> + Send {
        async {}
    }
}
