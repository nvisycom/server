//! [`HealthCheck`] implementation for [`NatsClient`].

use nvisy_core::health::{ComponentHealth, HealthCheck};

use super::NatsClient;
use crate::TRACING_TARGET_CONNECTION;

/// Component name reported for the NATS health check.
const COMPONENT_NAME: &str = "nats";

#[async_trait::async_trait]
impl HealthCheck for NatsClient {
    /// Probes NATS by checking the connection state, then pinging the server.
    async fn check_health(&self) -> ComponentHealth {
        if !self.is_connected() {
            tracing::warn!(target: TRACING_TARGET_CONNECTION, "NATS is not connected");
            return ComponentHealth::unhealthy(COMPONENT_NAME);
        }

        match self.ping().await {
            Ok(latency) => {
                tracing::debug!(
                    target: TRACING_TARGET_CONNECTION,
                    ping_ms = latency.as_millis(),
                    "NATS health check passed"
                );
                ComponentHealth::healthy(COMPONENT_NAME).with_latency(latency)
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_CONNECTION,
                    error = %e,
                    "NATS health check failed"
                );
                ComponentHealth::unhealthy(COMPONENT_NAME)
            }
        }
    }
}
