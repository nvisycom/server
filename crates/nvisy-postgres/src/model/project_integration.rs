//! Project integration model for PostgreSQL database operations.
//!
//! This module provides models for managing third-party integrations connected
//! to projects. Integrations enable projects to connect with external services
//! like version control systems, CI/CD pipelines, monitoring tools, and other
//! development workflow services.
//!
//! ## Models
//!
//! - [`ProjectIntegration`] - Main integration model with full configuration and status
//! - [`NewProjectIntegration`] - Data structure for creating new integrations
//! - [`UpdateProjectIntegration`] - Data structure for updating existing integrations

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::project_integrations;
use crate::types::{
    HasCreatedAt, HasLastActivityAt, HasOwnership, HasUpdatedAt, IntegrationStatus, IntegrationType,
};

/// Project integration model representing a third-party integration connected to a project.
///
/// This model manages connections between projects and external services, storing
/// configuration data, authentication credentials, and synchronization status.
/// Each integration maintains its own lifecycle with status tracking, error handling,
/// and activity monitoring to ensure reliable operation and easy troubleshooting.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = project_integrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ProjectIntegration {
    /// Unique integration identifier.
    pub id: Uuid,

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

/// Data structure for creating a new project integration.
///
/// Contains all the information necessary to set up a new integration with
/// an external service. Most fields are optional with sensible defaults,
/// allowing integrations to be created incrementally and configured over time.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = project_integrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewProjectIntegration {
    /// Reference to the project this integration will belong to.
    pub project_id: Uuid,

    /// Human-readable name for the integration.
    pub integration_name: String,

    /// Detailed description of the integration's purpose.
    pub description: String,

    /// Type of external service being integrated.
    pub integration_type: IntegrationType,

    /// Optional initial configuration and metadata.
    pub metadata: Option<serde_json::Value>,

    /// Optional authentication credentials for the external service.
    pub credentials: Option<serde_json::Value>,

    /// Whether the integration should be active immediately.
    pub is_active: Option<bool>,

    /// Optional timestamp of last synchronization.
    pub last_sync_at: Option<Timestamp>,

    /// Optional initial synchronization status.
    pub sync_status: Option<IntegrationStatus>,

    /// Account creating this integration.
    pub created_by: Uuid,
}

/// Data structure for updating an existing project integration.
///
/// Contains optional fields for modifying integration properties. Only the
/// fields that need to be changed should be set to Some(value), while
/// unchanged fields remain None to preserve their current values.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = project_integrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateProjectIntegration {
    /// Updated human-readable name for the integration.
    pub integration_name: Option<String>,

    /// Updated description of the integration's purpose.
    pub description: Option<String>,

    /// Updated type of external service being integrated.
    pub integration_type: Option<IntegrationType>,

    /// Updated configuration and service-specific metadata.
    pub metadata: Option<serde_json::Value>,

    /// Updated authentication credentials for the external service.
    pub credentials: Option<serde_json::Value>,

    /// Updated active status for the integration.
    pub is_active: Option<bool>,

    /// Updated timestamp of last successful synchronization.
    pub last_sync_at: Option<Timestamp>,

    /// Updated synchronization status.
    pub sync_status: Option<IntegrationStatus>,
}

impl ProjectIntegration {
    /// Returns whether the integration is currently in an error state.
    pub fn has_error(&self) -> bool {
        matches!(self.sync_status, Some(IntegrationStatus::Failed))
    }

    /// Returns whether the integration is waiting for initial setup completion.
    pub fn is_pending(&self) -> bool {
        matches!(self.sync_status, Some(IntegrationStatus::Pending))
    }

    /// Returns whether the integration is eligible for activation.
    pub fn can_activate(&self) -> bool {
        self.sync_status.is_none_or(|status| status.can_activate())
    }

    /// Returns whether the integration is fully operational and processing.
    pub fn is_operational(&self) -> bool {
        self.is_active && matches!(self.sync_status, Some(IntegrationStatus::Executing))
    }

    /// Returns whether the integration requires administrator attention.
    pub fn needs_attention(&self) -> bool {
        self.has_error() || !self.is_active
    }

    /// Returns whether the integration has completed at least one sync operation.
    pub fn has_been_synced(&self) -> bool {
        self.last_sync_at.is_some()
    }

    /// Returns whether the integration has configuration metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the integration has authentication credentials configured.
    pub fn has_credentials(&self) -> bool {
        !self
            .credentials
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the integration has all required configuration.
    pub fn is_configured(&self) -> bool {
        self.has_credentials() && !self.integration_name.is_empty()
    }

    /// Returns whether the integration's sync is overdue (>24 hours or never synced).
    pub fn is_sync_overdue(&self) -> bool {
        if let Some(duration) = self.time_since_last_activity() {
            duration.get_hours() > 24
        } else {
            true // Never synced
        }
    }

    /// Returns whether the integration is operating in a healthy state.
    ///
    /// A healthy integration is active, properly configured, free of errors,
    /// and has synced recently.
    pub fn is_healthy(&self) -> bool {
        self.is_active && !self.has_error() && self.is_configured() && !self.is_sync_overdue()
    }
}

impl HasCreatedAt for ProjectIntegration {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for ProjectIntegration {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasLastActivityAt for ProjectIntegration {
    fn last_activity_at(&self) -> Option<jiff::Timestamp> {
        self.last_sync_at.map(jiff::Timestamp::from)
    }
}

impl HasOwnership for ProjectIntegration {
    fn created_by(&self) -> Uuid {
        self.created_by
    }
}
