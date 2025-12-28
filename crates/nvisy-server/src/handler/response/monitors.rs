//! Monitor response types.

use jiff::Timestamp;
use nvisy_core::ServiceStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// System monitoring status response.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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
