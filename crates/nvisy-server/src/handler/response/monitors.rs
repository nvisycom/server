//! Monitor response types.

use jiff::Timestamp;
use nvisy_base::health::{ComponentHealth, HealthStatus};
use schemars::JsonSchema;
use serde::Serialize;

/// Response body for `GET /health`.
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
