//! The dependency-injected entry point for external object-store access.
//!
//! Bridges stored workspace connections to the [`providers`](crate::provider): a
//! connection carries an encrypted, typed
//! [`StorageConfig`](crate::provider::StorageConfig), which [`ExternalObjectStore`]
//! turns into a connected [`ObjectStoreClient`] at runtime.

use nvisy_core::net::EndpointPolicy;

use crate::client::ObjectStoreClient;
use crate::provider::{self, StorageConfig};

/// Tracing target for object storage operations.
const TRACING_TARGET: &str = "nvisy_object_store::client";

/// Connects workspace connections to their object storage backends.
///
/// Cloneable and cheap to pass around; it holds no per-connection state beyond
/// the deployment's [`EndpointPolicy`], and builds a fresh client per request
/// from the caller's credentials.
#[derive(Clone, Default)]
#[must_use = "service does nothing unless you use it"]
pub struct ExternalObjectStore {
    endpoint_policy: EndpointPolicy,
}

impl ExternalObjectStore {
    /// Creates a new [`ExternalObjectStore`] enforcing `endpoint_policy` on any
    /// caller-supplied custom endpoints.
    pub fn new(endpoint_policy: EndpointPolicy) -> Self {
        Self { endpoint_policy }
    }

    /// Connects to the object store described by the typed `StorageConfig`,
    /// validating any custom endpoint under the configured [`EndpointPolicy`].
    ///
    /// # Errors
    ///
    /// Returns an error if the endpoint is disallowed by the policy or the
    /// backend rejects the credentials.
    #[tracing::instrument(name = "object.connect", skip_all, fields(provider = %config.provider_id()))]
    pub async fn connect(&self, config: &StorageConfig) -> Result<ObjectStoreClient, crate::Error> {
        tracing::debug!(target: TRACING_TARGET, "Connecting to object store");
        provider::connect(config, self.endpoint_policy).await
    }
}
