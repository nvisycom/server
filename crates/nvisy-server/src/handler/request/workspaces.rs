//! Workspace request types.
//!
//! This module provides request DTOs for workspace management operations including
//! creation, updates, and archival. All request types support JSON serialization
//! and validation.

use nvisy_postgres::model::{
    NewWorkspace, UpdateWorkspace as UpdateWorkspaceModel, UpdateWorkspaceMember,
};
use nvisy_postgres::types::NotificationEvent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new workspace.
///
/// Creates a new workspace with the specified configuration. The creator is
/// automatically added as an owner of the workspace.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspace {
    /// Display name of the workspace (3-100 characters).
    #[validate(length(min = 3, max = 100))]
    pub display_name: String,
    /// Optional description of the workspace (max 200 characters).
    #[validate(length(max = 200))]
    pub description: Option<String>,
    /// Whether approval is required for processed files to be visible.
    pub require_approval: Option<bool>,
    /// Whether comments are enabled for this workspace.
    pub enable_comments: Option<bool>,
}

impl CreateWorkspace {
    /// Converts this request into a [`NewWorkspace`] model for database insertion.
    ///
    /// # Arguments
    ///
    /// * `account_id` - The ID of the account creating the workspace (becomes the owner).
    #[inline]
    pub fn into_model(self, account_id: Uuid) -> NewWorkspace {
        NewWorkspace {
            display_name: self.display_name,
            description: self.description,
            require_approval: self.require_approval,
            enable_comments: self.enable_comments,
            created_by: account_id,
            ..Default::default()
        }
    }
}

/// Request payload to update an existing workspace.
///
/// All fields are optional; only provided fields will be updated.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspace {
    /// New display name for the workspace (3-100 characters).
    #[validate(length(min = 3, max = 100))]
    pub display_name: Option<String>,
    /// New description for the workspace (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Whether approval is required for processed files to be visible.
    pub require_approval: Option<bool>,
    /// Whether comments are enabled for this workspace.
    pub enable_comments: Option<bool>,
}

impl UpdateWorkspace {
    pub fn into_model(self) -> UpdateWorkspaceModel {
        UpdateWorkspaceModel {
            display_name: self.display_name,
            description: self.description.map(Some),
            require_approval: self.require_approval,
            enable_comments: self.enable_comments,
            ..Default::default()
        }
    }
}

/// Request payload for updating notification settings.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotificationSettings {
    /// Whether to send email notifications.
    pub notify_via_email: Option<bool>,
    /// Notification events to receive in-app.
    pub notification_events_app: Option<Vec<NotificationEvent>>,
    /// Notification events to receive via email.
    pub notification_events_email: Option<Vec<NotificationEvent>>,
}

impl UpdateNotificationSettings {
    pub fn into_model(self) -> UpdateWorkspaceMember {
        UpdateWorkspaceMember {
            notify_via_email: self.notify_via_email,
            notification_events_app: self
                .notification_events_app
                .map(|events| events.into_iter().map(Some).collect()),
            notification_events_email: self
                .notification_events_email
                .map(|events| events.into_iter().map(Some).collect()),
            ..Default::default()
        }
    }
}
