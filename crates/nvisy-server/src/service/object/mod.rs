//! Object storage service.
//!
//! Bridges stored workspace connections to the [`nvisy_object`] providers: a
//! connection carries a provider id (e.g. `s3`) and encrypted credential JSON,
//! which this service turns into a connected [`ObjectStoreClient`] at runtime.

use nvisy_object::client::ObjectStoreClient;
use nvisy_object::providers;

mod bridge;
mod schedule;
mod sync;
mod worker;

pub use schedule::{is_cron_due, is_valid_cron};
pub use sync::ConnectionSyncService;
pub use worker::{ConnectionSyncJob, ConnectionSyncWorker};

/// Tracing target for object storage operations.
const TRACING_TARGET: &str = "nvisy_server::service::object";

/// Connects workspace connections to their object storage backends.
///
/// Cloneable and cheap to pass around; it holds no per-connection state and
/// builds a fresh client per request from the caller's credentials.
#[derive(Clone, Default)]
#[must_use = "service does nothing unless you use it"]
pub struct ObjectService;

impl ObjectService {
    /// Creates a new [`ObjectService`].
    pub fn new() -> Self {
        Self
    }

    /// Connects to the object store identified by `provider` using the given
    /// decrypted credential JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is unknown or the credentials do not
    /// match the provider's expected shape or are rejected by the backend.
    #[tracing::instrument(name = "object.connect", skip_all, fields(provider = %provider))]
    pub async fn connect(
        &self,
        provider: &str,
        credentials: serde_json::Value,
    ) -> Result<ObjectStoreClient, nvisy_object::error::Error> {
        tracing::debug!(target: TRACING_TARGET, "Connecting to object store");
        providers::connect(provider, credentials).await
    }
}
