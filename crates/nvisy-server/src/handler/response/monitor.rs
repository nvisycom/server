//! Monitor response types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

/// System monitoring status response with health information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "updatedAt": "2023-12-07T10:30:00Z",
    "isHealthy": true
}))]
pub struct MonitorStatusResponse {
    /// Timestamp when this status was generated.
    pub updated_at: OffsetDateTime,
    /// Overall system health status.
    pub is_healthy: bool,
}
