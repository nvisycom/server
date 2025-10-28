//! Monitor response types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;

use super::super::monitors::ComponentStatus;
use crate::service::DataCollectionPolicy;

/// Feature monitoring state with health information.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FeatureState {
    /// Whether the feature is operational
    pub is_operational: bool,
    /// Human-readable status description
    pub status: String,
    /// Optional status message with additional details
    pub message: Option<String>,
}

impl FeatureState {
    /// Creates a new [`FeatureState`].
    #[inline]
    pub fn new(status: ComponentStatus) -> Self {
        Self {
            is_operational: status.is_operational(),
            status: if status.is_healthy {
                "healthy".to_string()
            } else {
                "unhealthy".to_string()
            },
            message: status.message,
        }
    }
}

/// Detailed system component statuses (requires authentication).
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeatureStatuses {
    /// Database and API gateway server status.
    pub gateway_server: FeatureState,
    /// Background worker runtime status.
    pub worker_runtime: FeatureState,
    /// AI assistant chat service status.
    pub assistant_chat: FeatureState,
}

/// System monitoring status response with optional detailed component information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatusResponse {
    /// Current regional data collection policy in effect.
    pub regional_policy: DataCollectionPolicy,
    /// Timestamp when this status was generated.
    pub updated_at: OffsetDateTime,
    /// Detailed component statuses (only included for authenticated requests).
    #[serde(flatten)]
    pub features: Option<FeatureStatuses>,
}
