//! Monitor request types.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Default timeout for health checks in milliseconds.
const DEFAULT_TIMEOUT_MS: u32 = 5000;

/// Request payload for monitoring status endpoint.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CheckHealth {
    /// Timeout in milliseconds for health checks.
    #[validate(range(min = 100, max = 30000))]
    pub timeout: Option<u32>,
    /// Whether to return cached results if available.
    pub use_cache: Option<bool>,
}

impl CheckHealth {
    /// Returns the timeout duration for health checks.
    ///
    /// Uses the configured timeout or falls back to the default of 5 seconds.
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.timeout.unwrap_or(DEFAULT_TIMEOUT_MS) as u64)
    }
}
