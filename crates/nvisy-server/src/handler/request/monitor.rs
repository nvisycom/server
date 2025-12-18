//! Monitor request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request payload for monitoring status endpoint.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "includeDetails": true,
    "services": ["database", "cache"],
    "timeout": 5000
}))]
pub struct CheckHealth {
    /// Timeout in milliseconds for health checks.
    #[validate(range(min = 100, max = 30000))]
    pub timeout: Option<u32>,

    /// Whether to return cached results if available.
    pub use_cache: Option<bool>,
}
