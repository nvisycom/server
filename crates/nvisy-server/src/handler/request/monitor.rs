//! Monitor request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request payload for monitoring status endpoint.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "dataCollection": "minimal"
}))]
pub struct GetMonitorStatus {
    /// Whether to return cached health status.
    pub return_cached: Option<bool>,
}
