//! [`HealthCheck`] implementation for [`BlobStore`].

use nvisy_core::health::{ComponentHealth, HealthCheck};

use super::BlobStore;

/// Component name reported for the S3 health check.
const COMPONENT_NAME: &str = "s3";

#[async_trait::async_trait]
impl HealthCheck for BlobStore {
    /// Probes the blob store by heading its configured bucket.
    async fn check_health(&self) -> ComponentHealth {
        ComponentHealth::from_probe(COMPONENT_NAME, self.ping().await)
    }
}
