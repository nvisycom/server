//! [`HealthCheck`] implementation for [`WebhookService`].

use nvisy_core::health::{ComponentHealth, HealthCheck};

use super::WebhookService;

#[async_trait::async_trait]
impl HealthCheck for WebhookService {
    /// Reports the health of the underlying webhook provider.
    async fn check_health(&self) -> ComponentHealth {
        self.inner.check_health().await
    }
}
