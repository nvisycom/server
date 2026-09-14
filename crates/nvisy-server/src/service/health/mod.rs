//! Health monitoring service.
//!
//! [`HealthService`] aggregates the [`HealthCheck`] results of all registered
//! components, caching them with a TTL to balance responsiveness against the cost
//! of repeated probes. It hands back a [`HealthReading`] (a [`HealthReport`] plus
//! a timestamp); mapping that onto the HTTP response is the handler's concern.
//!
//! [`HealthCheck`]: nvisy_core::health::HealthCheck
//! [`HealthReport`]: nvisy_core::health::HealthReport

use std::time::Duration;

mod service;

pub use service::{HealthReading, HealthService};

/// Tracing target for health monitoring operations.
const TRACING_TARGET: &str = "nvisy_server::service::health";

/// Default cache duration for health checks.
pub const DEFAULT_CACHE_DURATION: Duration = Duration::from_secs(30);

/// Health monitoring configuration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
#[must_use = "config does nothing unless you use it"]
pub struct HealthConfig {
    /// How long cached health results remain valid before a fresh check.
    #[cfg_attr(
        feature = "cli",
        arg(
            long = "health-cache-duration",
            env = "HEALTH_CACHE_DURATION",
            default_value = "30s",
            value_parser = humantime::parse_duration,
        )
    )]
    pub cache_duration: Duration,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            cache_duration: DEFAULT_CACHE_DURATION,
        }
    }
}
