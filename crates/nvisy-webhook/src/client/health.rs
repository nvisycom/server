//! [`HealthCheck`] implementation for [`WebhookService`].

use nvisy_base::health::{ComponentHealth, HealthCheck};

use super::WebhookService;

/// Component name reported for the webhook health check.
const COMPONENT_NAME: &str = "webhook";

#[async_trait::async_trait]
impl HealthCheck for WebhookService {
    /// Probes the webhook provider via its health check.
    async fn check_health(&self) -> ComponentHealth {
        self.health_check()
            .await
            .unwrap_or_else(|_| ComponentHealth::unhealthy(COMPONENT_NAME))
    }
}
