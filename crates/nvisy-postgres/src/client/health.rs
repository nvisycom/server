//! [`HealthCheck`] implementation for [`PgClient`].

use nvisy_base::health::{ComponentHealth, HealthCheck};

use super::PgClient;
use crate::TRACING_TARGET_CONNECTION;

/// Component name reported for the Postgres health check.
const COMPONENT_NAME: &str = "postgres";

#[async_trait::async_trait]
impl HealthCheck for PgClient {
    /// Probes Postgres by acquiring a connection from the pool.
    async fn check_health(&self) -> ComponentHealth {
        match self.get_connection().await {
            Ok(_) => {
                tracing::debug!(target: TRACING_TARGET_CONNECTION, "Postgres health check passed");
                ComponentHealth::healthy(COMPONENT_NAME)
            }
            Err(e) => {
                tracing::warn!(
                    target: TRACING_TARGET_CONNECTION,
                    error = %e,
                    "Postgres health check failed"
                );
                ComponentHealth::unhealthy(COMPONENT_NAME)
            }
        }
    }
}
