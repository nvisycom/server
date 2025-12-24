//! Project integration request types.

use nvisy_postgres::types::IntegrationType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request payload for creating a new project integration.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectIntegration {
    /// Human-readable name for the integration.
    #[validate(length(min = 1, max = 100))]
    pub integration_name: String,

    /// Detailed description of the integration's purpose and functionality.
    #[validate(length(min = 1, max = 500))]
    pub description: String,

    /// Type of third-party service this integration connects to.
    pub integration_type: IntegrationType,

    /// Optional structured configuration and service-specific metadata.
    pub metadata: Option<serde_json::Value>,

    /// Optional authentication credentials for the external service.
    pub credentials: Option<serde_json::Value>,

    /// Whether the integration should be active immediately upon creation.
    pub is_active: Option<bool>,
}

/// Request payload for updating an existing project integration.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectIntegration {
    /// Updated human-readable name for the integration.
    #[validate(length(min = 1, max = 100))]
    pub integration_name: Option<String>,

    /// Updated description of the integration's purpose.
    #[validate(length(min = 1, max = 500))]
    pub description: Option<String>,

    /// Updated type of external service being integrated.
    pub integration_type: Option<IntegrationType>,

    /// Updated configuration and service-specific metadata.
    pub metadata: Option<serde_json::Value>,

    /// Updated authentication credentials for the external service.
    pub credentials: Option<serde_json::Value>,

    /// Updated active status for the integration.
    pub is_active: Option<bool>,
}

/// Request payload for updating integration credentials only.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIntegrationCredentials {
    /// Updated authentication credentials for the external service.
    pub credentials: serde_json::Value,
}
