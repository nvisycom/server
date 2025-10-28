//! Monitor request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::service::DataCollectionPolicy;

/// Request payload for monitoring status endpoint.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "preferPolicy": "Eu"
}))]
pub struct MonitorStatusRequest {
    /// Preferred regional policy for data collection.
    pub prefer_policy: Option<DataCollectionPolicy>,
}
