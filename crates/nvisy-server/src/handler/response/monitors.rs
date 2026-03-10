//! Monitor response types.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents the operational status of a service.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Service is operating normally.
    #[default]
    Healthy,
    /// Service is operating with some issues but still functional.
    Degraded,
    /// Service is not operational.
    Unhealthy,
}

/// System monitoring status response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatus {
    /// Timestamp when this status was generated.
    pub checked_at: Timestamp,
    /// Overall system health status.
    pub status: ServiceStatus,
    /// Application version.
    pub version: String,
}

impl Default for MonitorStatus {
    fn default() -> Self {
        Self {
            checked_at: Timestamp::now(),
            status: ServiceStatus::Healthy,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
