//! Health monitoring service.
//!
//! Aggregates the [`HealthCheck`](nvisy_base::health::HealthCheck) results of
//! all registered components ([`HealthCache`]), caching them with a TTL to
//! balance responsiveness against the cost of repeated probes.

use std::time::Duration;

mod cache;
mod snapshot;

pub use cache::HealthCache;

/// Tracing target for health monitoring operations.
const TRACING_TARGET: &str = "nvisy_server::health";

/// Default cache duration for health checks.
pub const DEFAULT_CACHE_DURATION: Duration = Duration::from_secs(30);

/// Health monitoring configuration.
#[derive(Debug, Clone)]
#[must_use = "config does nothing unless you use it"]
pub struct HealthConfig {
    /// How long cached health results remain valid before a fresh check.
    pub cache_duration: Duration,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            cache_duration: DEFAULT_CACHE_DURATION,
        }
    }
}
