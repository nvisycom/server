//! [`HealthCheck`] implementation for [`PgClient`].

use nvisy_core::health::{ComponentHealth, HealthCheck};

use super::PgClient;

/// Component name reported for the Postgres health check.
const COMPONENT_NAME: &str = "postgres";

#[async_trait::async_trait]
impl HealthCheck for PgClient {
    /// Probes Postgres by acquiring a connection from the pool.
    async fn check_health(&self) -> ComponentHealth {
        ComponentHealth::from_probe(COMPONENT_NAME, self.get_connection().await)
    }
}
