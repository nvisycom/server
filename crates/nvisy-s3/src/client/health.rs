//! [`HealthCheck`] implementation for [`BlobStore`].

use nvisy_core::health::{ComponentHealth, HealthCheck};

use super::BlobStore;

/// Tracing target for blob-store health checks.
const TRACING_TARGET: &str = "nvisy_s3::health";

/// Component name reported for the blob-store health check.
const COMPONENT_NAME: &str = "s3";

#[async_trait::async_trait]
impl HealthCheck for BlobStore {
    /// Probes the blob store by heading its configured bucket.
    async fn check_health(&self) -> ComponentHealth {
        match self.ping().await {
            Ok(()) => {
                tracing::debug!(target: TRACING_TARGET, "Blob store health check passed");
                ComponentHealth::healthy(COMPONENT_NAME)
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %e,
                    "Blob store health check failed"
                );
                ComponentHealth::unhealthy(COMPONENT_NAME)
            }
        }
    }
}
