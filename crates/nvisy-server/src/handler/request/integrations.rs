//! Project integration request types.
//!
//! This module provides request DTOs for project integration management including
//! creation, updates, and credential management.

use nvisy_postgres::model::{
    NewProjectIntegration, UpdateProjectIntegration as UpdateProjectIntegrationModel,
};
use nvisy_postgres::types::IntegrationType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new project integration.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectIntegration {
    /// Human-readable name for the integration (1-100 characters).
    #[validate(length(min = 1, max = 100))]
    pub integration_name: String,

    /// Detailed description of the integration's purpose (1-500 characters).
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

impl CreateProjectIntegration {
    /// Converts this request into a [`NewProjectIntegration`] model.
    ///
    /// # Arguments
    ///
    /// * `project_id` - The project this integration belongs to.
    /// * `account_id` - The account creating the integration.
    #[inline]
    pub fn into_model(self, project_id: Uuid, account_id: Uuid) -> NewProjectIntegration {
        NewProjectIntegration {
            project_id,
            integration_name: self.integration_name,
            description: self.description,
            integration_type: self.integration_type,
            metadata: self.metadata,
            credentials: self.credentials,
            is_active: self.is_active,
            last_sync_at: None,
            sync_status: None,
            created_by: account_id,
        }
    }
}

/// Request payload for updating an existing project integration.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectIntegration {
    /// Updated human-readable name for the integration (1-100 characters).
    #[validate(length(min = 1, max = 100))]
    pub integration_name: Option<String>,

    /// Updated description of the integration's purpose (1-500 characters).
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

impl UpdateProjectIntegration {
    /// Converts this request into an [`UpdateProjectIntegrationModel`].
    #[inline]
    pub fn into_model(self) -> UpdateProjectIntegrationModel {
        UpdateProjectIntegrationModel {
            integration_name: self.integration_name,
            description: self.description,
            integration_type: self.integration_type,
            metadata: self.metadata,
            credentials: self.credentials,
            is_active: self.is_active,
            last_sync_at: None,
            sync_status: None,
        }
    }
}

/// Request payload for updating integration credentials only.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIntegrationCredentials {
    /// Updated authentication credentials for the external service.
    pub credentials: serde_json::Value,
}
