//! Project integration model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::project_integrations;
use crate::types::{IntegrationStatus, IntegrationType};

/// Project integration model representing a third-party integration connected to a project.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_integrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectIntegration {
    /// Unique integration identifier
    pub id: Uuid,
    /// Reference to the project
    pub project_id: Uuid,
    /// Human-readable integration name
    pub integration_name: String,
    /// Description of what this integration does
    pub description: String,
    /// Type of integration
    pub integration_type: IntegrationType,
    /// Configuration and metadata
    pub metadata: serde_json::Value,
    /// Authentication credentials
    pub credentials: serde_json::Value,
    /// Whether the integration is active
    pub is_active: bool,
    /// Last sync timestamp
    pub last_sync_at: Option<OffsetDateTime>,
    /// Current sync status
    pub sync_status: Option<IntegrationStatus>,
    /// Account that created the integration
    pub created_by: Uuid,
    /// Timestamp when the integration was created
    pub created_at: OffsetDateTime,
    /// Timestamp when the integration was last updated
    pub updated_at: OffsetDateTime,
}

/// Data for creating a new project integration.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = project_integrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectIntegration {
    /// Project ID
    pub project_id: Uuid,
    /// Integration name
    pub integration_name: String,
    /// Integration description
    pub description: String,
    /// Integration type
    pub integration_type: IntegrationType,
    /// Configuration and metadata
    pub metadata: Option<serde_json::Value>,
    /// Credentials
    pub credentials: Option<serde_json::Value>,
    /// Is active
    pub is_active: Option<bool>,
    /// Last sync at
    pub last_sync_at: Option<OffsetDateTime>,
    /// Sync status
    pub sync_status: Option<IntegrationStatus>,
    /// Created by
    pub created_by: Uuid,
}

/// Data for updating a project integration.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_integrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectIntegration {
    /// Integration name
    pub integration_name: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Integration type
    pub integration_type: Option<IntegrationType>,
    /// Configuration and metadata
    pub metadata: Option<serde_json::Value>,
    /// Credentials
    pub credentials: Option<serde_json::Value>,
    /// Is active
    pub is_active: Option<bool>,
    /// Last sync at
    pub last_sync_at: Option<OffsetDateTime>,
    /// Sync status
    pub sync_status: Option<IntegrationStatus>,
}

impl ProjectIntegration {
    /// Returns whether the integration has errors.
    pub fn has_error(&self) -> bool {
        matches!(self.sync_status, Some(IntegrationStatus::Failure))
    }

    /// Returns whether the integration is pending setup.
    pub fn is_pending(&self) -> bool {
        matches!(self.sync_status, Some(IntegrationStatus::Pending))
    }

    /// Returns whether the integration can be activated.
    pub fn can_activate(&self) -> bool {
        self.sync_status
            .map_or(true, |status| status.can_activate())
    }

    /// Returns whether the integration is operational (active and executing).
    pub fn is_operational(&self) -> bool {
        self.is_active && matches!(self.sync_status, Some(IntegrationStatus::Executing))
    }

    /// Returns whether the integration needs attention (has errors or is disabled).
    pub fn needs_attention(&self) -> bool {
        self.has_error() || !self.is_active
    }

    /// Returns whether the integration was synced recently (within last hour).
    pub fn is_recently_synced(&self) -> bool {
        if let Some(last_sync) = self.last_sync_at {
            let now = OffsetDateTime::now_utc();
            let duration = now - last_sync;
            duration.whole_hours() < 1
        } else {
            false
        }
    }
}
