//! Workspace integration response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{IntegrationStatus, IntegrationType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Workspace integration response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Integration {
    /// Unique integration identifier.
    pub integration_id: Uuid,

    /// Reference to the workspace this integration belongs to.
    pub workspace_id: Uuid,

    /// Human-readable name for the integration.
    pub integration_name: String,

    /// Detailed description of the integration's purpose and functionality.
    pub description: String,

    /// Type of third-party service this integration connects to.
    pub integration_type: IntegrationType,

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

impl Integration {
    pub fn from_model(integration: model::WorkspaceIntegration) -> Self {
        Self {
            integration_id: integration.id,
            workspace_id: integration.workspace_id,
            integration_name: integration.integration_name,
            description: integration.description,
            integration_type: integration.integration_type,
            is_active: integration.is_active,
            last_sync_at: integration.last_sync_at.map(Into::into),
            sync_status: integration.sync_status,
            created_by: integration.created_by,
            created_at: integration.created_at.into(),
            updated_at: integration.updated_at.into(),
        }
    }
}

/// Paginated response for workspace integrations.
pub type IntegrationsPage = Page<Integration>;
