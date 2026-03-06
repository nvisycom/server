//! Health check result for webhook providers.

use std::time::Duration;

/// Health check result from a webhook provider.
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    healthy: bool,
    /// How long the health check took.
    pub latency: Duration,
}

impl ServiceHealth {
    /// Creates a healthy result.
    pub fn healthy(latency: Duration) -> Self {
        Self {
            healthy: true,
            latency,
        }
    }

    /// Creates an unhealthy result.
    pub fn unhealthy(latency: Duration) -> Self {
        Self {
            healthy: false,
            latency,
        }
    }

    /// Returns whether the service is operational.
    pub fn is_healthy(&self) -> bool {
        self.healthy
    }
}
