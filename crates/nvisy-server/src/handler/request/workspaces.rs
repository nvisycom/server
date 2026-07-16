//! Workspace request types.
//!
//! This module provides request DTOs for workspace management operations including
//! creation, updates, and archival. All request types support JSON serialization
//! and validation.

use nvisy_postgres::model::{
    NewWorkspace, UpdateWorkspace as UpdateWorkspaceModel, UpdateWorkspaceMember,
};
use nvisy_postgres::types::{NotificationEvent, WorkspaceSlug};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::handler::{ErrorKind, Result};

/// Request payload for creating a new workspace.
///
/// Creates a new workspace with the specified configuration. The creator is
/// automatically added as an owner of the workspace.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspace {
    /// Display name of the workspace (3-32 characters).
    #[validate(length(min = 3, max = 32))]
    pub display_name: String,
    /// Optional URL slug. Derived from the display name when omitted.
    pub slug: Option<WorkspaceSlug>,
    /// Optional description of the workspace (max 200 characters).
    #[validate(length(max = 200))]
    pub description: Option<String>,
    /// Whether approval is required for processed files to be visible.
    pub require_approval: Option<bool>,
}

impl CreateWorkspace {
    /// Converts this request into a [`NewWorkspace`] model for database insertion.
    ///
    /// The slug is the caller-provided one, or derived from the display name.
    /// The returned slug is only the *preferred* value; the repository resolves
    /// collisions with a numeric suffix on insert.
    ///
    /// # Arguments
    ///
    /// * `account_id` - The ID of the account creating the workspace (becomes the owner).
    ///
    /// # Errors
    ///
    /// Returns `BadRequest` if no slug was given and the display name has no
    /// slug-able characters.
    pub fn into_model(self, account_id: Uuid) -> Result<NewWorkspace> {
        let slug = match self.slug {
            Some(slug) => slug,
            None => WorkspaceSlug::derive(&self.display_name).ok_or_else(|| {
                ErrorKind::BadRequest
                    .with_message("Could not derive a slug from the display name; provide one")
                    .with_resource("workspace")
            })?,
        };

        Ok(NewWorkspace {
            display_name: self.display_name,
            slug,
            description: self.description,
            avatar_url: None,
            require_approval: self.require_approval,
            tags: None,
            metadata: None,
            settings: None,
            created_by: account_id,
        })
    }
}

/// Request payload to update an existing workspace.
///
/// All fields are optional; only provided fields will be updated.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspace {
    /// New display name for the workspace (3-32 characters).
    #[validate(length(min = 3, max = 32))]
    pub display_name: Option<String>,
    /// New URL slug for the workspace.
    pub slug: Option<WorkspaceSlug>,
    /// New description for the workspace (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Whether approval is required for processed files to be visible.
    pub require_approval: Option<bool>,
}

impl UpdateWorkspace {
    pub fn into_model(self) -> UpdateWorkspaceModel {
        UpdateWorkspaceModel {
            display_name: self.display_name,
            slug: self.slug,
            description: self.description.map(Some),
            require_approval: self.require_approval,
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
