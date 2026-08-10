//! Workspace request types.
//!
//! This module provides request DTOs for workspace management operations including
//! creation, updates, and archival. All request types support JSON serialization
//! and validation.

use nvisy_postgres::model::{
    NewWorkspace, UpdateWorkspace as UpdateWorkspaceModel, UpdateWorkspaceMember,
};
use nvisy_postgres::types::{Handle, NotificationEvent, WorkspaceSettings};
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
    /// Display name of the workspace (2-32 characters).
    #[validate(length(min = 2, max = 32))]
    pub display_name: String,
    /// Optional URL slug. Derived from the display name when omitted.
    pub slug: Option<Handle>,
    /// Optional description of the workspace (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Workspace settings (approval requirement, data-retention rules). Defaults
    /// to requiring approval and keeping everything when omitted.
    pub settings: Option<WorkspaceSettings>,
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
            None => Handle::derive(&self.display_name).ok_or_else(|| {
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
            tags: None,
            metadata: None,
            settings: self
                .settings
                .map(|settings| settings.to_value())
                .transpose()
                .map_err(|err| {
                    ErrorKind::BadRequest
                        .with_message("Invalid workspace settings")
                        .with_context(err.to_string())
                })?,
            created_by: account_id,
        })
    }
}

/// Request payload to update an existing workspace.
///
/// All fields are optional; only provided fields will be updated. The slug is
/// immutable and set at creation, so it cannot be changed here.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspace {
    /// New display name for the workspace (2-32 characters).
    #[validate(length(min = 2, max = 32))]
    pub display_name: Option<String>,
    /// New description for the workspace (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Replacement workspace settings (approval requirement, data-retention
    /// rules). When omitted, settings are left unchanged.
    pub settings: Option<WorkspaceSettings>,
}

impl UpdateWorkspace {
    pub fn into_model(self) -> Result<UpdateWorkspaceModel> {
        let settings = self
            .settings
            .map(|settings| settings.to_value())
            .transpose()
            .map_err(|err| {
                ErrorKind::BadRequest
                    .with_message("Invalid workspace settings")
                    .with_context(err.to_string())
            })?;

        Ok(UpdateWorkspaceModel {
            display_name: self.display_name,
            description: self.description.map(Some),
            settings,
            ..Default::default()
        })
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
