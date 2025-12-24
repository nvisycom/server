//! Project integration response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{IntegrationStatus, IntegrationType};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use uuid::Uuid;

/// Project integration response.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIntegration {
    /// Unique integration identifier.
    pub integration_id: Uuid,

    /// Reference to the project this integration belongs to.
    pub project_id: Uuid,

    /// Human-readable name for the integration.
    pub integration_name: String,

    /// Detailed description of the integration's purpose and functionality.
    pub description: String,

    /// Type of third-party service this integration connects to.
    pub integration_type: IntegrationType,

    /// Structured configuration and service-specific metadata.
    pub metadata: serde_json::Value,

    /// Whether the integration is currently active and enabled.
    pub is_active: bool,

    /// Timestamp of the most recent successful synchronization.
    pub last_sync_at: Option<Timestamp>,

    /// Current status of synchronization operations.
    pub sync_status: Option<IntegrationStatus>,

    /// Account that originally created this integration.
    pub created_by: Uuid,

    /// Timestamp when this integration was first created.
    pub created_at: Timestamp,

    /// Timestamp when this integration was last modified.
    pub updated_at: Timestamp,
}

/// Project integration response with credentials (for sensitive operations).
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIntegrationWithCredentials {
    /// Unique integration identifier.
    pub integration_id: Uuid,

    /// Reference to the project this integration belongs to.
    pub project_id: Uuid,

    /// Human-readable name for the integration.
    pub integration_name: String,

    /// Detailed description of the integration's purpose and functionality.
    pub description: String,

    /// Type of third-party service this integration connects to.
    pub integration_type: IntegrationType,

    /// Structured configuration and service-specific metadata.
    pub metadata: serde_json::Value,

    /// Encrypted authentication credentials for the external service.
    pub credentials: serde_json::Value,

    /// Whether the integration is currently active and enabled.
    pub is_active: bool,

    /// Timestamp of the most recent successful synchronization.
    pub last_sync_at: Option<Timestamp>,

    /// Current status of synchronization operations.
    pub sync_status: Option<IntegrationStatus>,

    /// Account that originally created this integration.
    pub created_by: Uuid,

    /// Timestamp when this integration was first created.
    pub created_at: Timestamp,

    /// Timestamp when this integration was last modified.
    pub updated_at: Timestamp,
}

/// Summary information about a project integration for list views.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIntegrationSummary {
    /// Unique integration identifier.
    pub integration_id: Uuid,

    /// Reference to the project this integration belongs to.
    pub project_id: Uuid,

    /// Human-readable name for the integration.
    pub integration_name: String,

    /// Type of third-party service this integration connects to.
    pub integration_type: IntegrationType,

    /// Whether the integration is currently active and enabled.
    pub is_active: bool,

    /// Current status of synchronization operations.
    pub sync_status: Option<IntegrationStatus>,

    /// Timestamp of the most recent successful synchronization.
    pub last_sync_at: Option<Timestamp>,

    /// Timestamp when this integration was first created.
    pub created_at: Timestamp,
}

impl ProjectIntegration {
    /// Creates a new instance of [`ProjectIntegration`] from database model.
    pub fn new(integration: model::ProjectIntegration) -> Self {
        Self {
            integration_id: integration.id,
            project_id: integration.project_id,
            integration_name: integration.integration_name,
            description: integration.description,
            integration_type: integration.integration_type,
            metadata: integration.metadata,
            is_active: integration.is_active,
            last_sync_at: integration.last_sync_at.map(Into::into),
            sync_status: integration.sync_status,
            created_by: integration.created_by,
            created_at: integration.created_at.into(),
            updated_at: integration.updated_at.into(),
        }
    }
}

impl ProjectIntegrationWithCredentials {
    /// Creates a new instance of [`ProjectIntegrationWithCredentials`] from database model.
    pub fn from_model(integration: model::ProjectIntegration) -> Self {
        Self {
            integration_id: integration.id,
            project_id: integration.project_id,
            integration_name: integration.integration_name,
            description: integration.description,
            integration_type: integration.integration_type,
            metadata: integration.metadata,
            credentials: integration.credentials,
            is_active: integration.is_active,
            last_sync_at: integration.last_sync_at.map(Into::into),
            sync_status: integration.sync_status,
            created_by: integration.created_by,
            created_at: integration.created_at.into(),
            updated_at: integration.updated_at.into(),
        }
    }
}

impl ProjectIntegrationSummary {
    /// Creates a new instance of [`ProjectIntegrationSummary`] from database model.
    pub fn from_model(integration: model::ProjectIntegration) -> Self {
        Self {
            integration_id: integration.id,
            project_id: integration.project_id,
            integration_name: integration.integration_name,
            integration_type: integration.integration_type,
            is_active: integration.is_active,
            sync_status: integration.sync_status,
            last_sync_at: integration.last_sync_at.map(Into::into),
            created_at: integration.created_at.into(),
        }
    }
}

impl From<model::ProjectIntegration> for ProjectIntegration {
    #[inline]
    fn from(integration: model::ProjectIntegration) -> Self {
        Self::new(integration)
    }
}

impl From<model::ProjectIntegration> for ProjectIntegrationWithCredentials {
    #[inline]
    fn from(integration: model::ProjectIntegration) -> Self {
        Self::from_model(integration)
    }
}

impl From<model::ProjectIntegration> for ProjectIntegrationSummary {
    #[inline]
    fn from(integration: model::ProjectIntegration) -> Self {
        Self::from_model(integration)
    }
}

/// Response for listing project integrations.
pub type ProjectIntegrations = Vec<ProjectIntegration>;

/// Response for listing project integration summaries.
pub type ProjectIntegrationSummaries = Vec<ProjectIntegrationSummary>;
