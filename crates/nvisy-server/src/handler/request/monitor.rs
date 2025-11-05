//! Monitor request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::service::DataCollectionPolicy;

/// Request payload for monitoring status endpoint.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "dataCollection": "minimal"
}))]
pub struct MonitorStatusRequest {
    /// Preferred data collection policy.
    pub data_collection: Option<DataCollectionPolicy>,
    /// Whether to return cached health status.
    pub return_cached: Option<bool>,
}
