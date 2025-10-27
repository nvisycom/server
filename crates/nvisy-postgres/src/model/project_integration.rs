//! Project integration model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::project_integrations;
use crate::types::IntegrationStatus;

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
    /// Current operational status
    pub status: IntegrationStatus,
    /// Whether the integration is enabled
    pub is_enabled: bool,
    /// Authentication data (credentials, tokens, etc.)
    pub auth_data: serde_json::Value,
    /// Additional metadata and configuration
    pub metadata: serde_json::Value,
    /// Account that created the integration
    pub created_by: Uuid,
    /// Account that last updated the integration
    pub updated_by: Uuid,
    /// Timestamp when the integration was created
    pub created_at: OffsetDateTime,
    /// Timestamp when the integration was last updated
    pub updated_at: OffsetDateTime,
    /// Timestamp when the integration was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new project integration.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = project_integrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectIntegration {
    /// Project ID
    pub project_id: Uuid,
    /// Integration name
    pub integration_name: String,
    /// Integration description
    pub description: String,
    /// Integration status
    pub status: IntegrationStatus,
    /// Is enabled
    pub is_enabled: bool,
    /// Authentication data
    pub auth_data: serde_json::Value,
    /// Metadata
    pub metadata: serde_json::Value,
    /// Created by
    pub created_by: Uuid,
    /// Updated by
    pub updated_by: Uuid,
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
    /// Status
    pub status: Option<IntegrationStatus>,
    /// Is enabled
    pub is_enabled: Option<bool>,
    /// Authentication data
    pub auth_data: Option<serde_json::Value>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Updated by
    pub updated_by: Option<Uuid>,
}

impl Default for NewProjectIntegration {
    fn default() -> Self {
        Self {
            project_id: Uuid::new_v4(),
            integration_name: String::new(),
            description: String::new(),
            status: IntegrationStatus::Pending,
            is_enabled: true,
            auth_data: serde_json::Value::Object(serde_json::Map::new()),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            created_by: Uuid::new_v4(),
            updated_by: Uuid::new_v4(),
        }
    }
}

impl ProjectIntegration {
    /// Returns whether the integration is currently active.
    pub fn is_active(&self) -> bool {
        self.deleted_at.is_none() && self.is_enabled && self.status == IntegrationStatus::Executing
    }

    /// Returns whether the integration is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the integration has errors.
    pub fn has_error(&self) -> bool {
        self.status == IntegrationStatus::Failure
    }

    /// Returns whether the integration is pending setup.
    pub fn is_pending(&self) -> bool {
        self.status == IntegrationStatus::Pending
    }

    /// Returns whether the integration can be activated.
    pub fn can_activate(&self) -> bool {
        !self.is_deleted() && self.status.can_activate()
    }

    /// Returns whether the integration can be configured.
    pub fn can_configure(&self) -> bool {
        !self.is_deleted()
    }

    /// Returns whether the integration is operational (active and executing).
    pub fn is_operational(&self) -> bool {
        self.is_active() && self.status.is_operational()
    }

    /// Returns whether the integration needs attention (has errors or is disabled).
    pub fn needs_attention(&self) -> bool {
        self.has_error() || (!self.is_enabled && !self.is_deleted())
    }
}
