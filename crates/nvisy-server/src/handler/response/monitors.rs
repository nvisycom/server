//! Monitor response types.

use jiff::Timestamp;
use nvisy_core::health::{ComponentHealth, HealthStatus};
use schemars::JsonSchema;
use serde::Serialize;

use crate::service::HealthReading;

/// Response body for `GET /health/`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// Overall service status.
    pub status: HealthStatus,
    /// Per-component health checks.
    pub checks: Vec<ComponentHealth>,
    /// RFC 3339 timestamp of when the check was performed.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
}

impl From<HealthReading> for Health {
    fn from(reading: HealthReading) -> Self {
        Self {
            status: reading.report.status,
            checks: reading.report.components,
            timestamp: reading.timestamp,
        }
    }
}
