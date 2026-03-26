//! Monitor request types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request payload for monitoring status endpoint.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CheckHealth {
    /// Whether to return cached results if available.
    pub use_cache: Option<bool>,
}
