//! Workspace webhook request types.
//!
//! This module provides request DTOs for workspace webhook management including
//! creation and updates.

use nvisy_postgres::model::{NewWorkspaceWebhook, UpdateWorkspaceWebhook as UpdateWorkspaceWebhookModel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new workspace webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebhook {
    /// Human-readable name for the webhook (1-100 characters).
    #[validate(length(min = 1, max = 100))]
    pub display_name: String,

    /// Detailed description of the webhook's purpose (max 500 characters).
    #[validate(length(max = 500))]
    #[serde(default)]
    pub description: String,

    /// The URL to send webhook payloads to.
    #[validate(url, length(min = 1, max = 2048))]
    pub url: String,

    /// Optional secret for signing webhook payloads (max 256 characters).
    #[validate(length(max = 256))]
    pub secret: Option<String>,

    /// List of event types this webhook should receive.
    pub events: Vec<String>,

    /// Optional custom headers to include in webhook requests.
    pub headers: Option<serde_json::Value>,

    /// Maximum number of consecutive failures before disabling (1-100).
    #[validate(range(min = 1, max = 100))]
    pub max_failures: Option<i32>,
}

impl CreateWebhook {
    /// Converts this request into a [`NewWorkspaceWebhook`] model.
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - The workspace this webhook belongs to.
    /// * `account_id` - The account creating the webhook.
    #[inline]
    pub fn into_model(self, workspace_id: Uuid, account_id: Uuid) -> NewWorkspaceWebhook {
        let events = self.events.into_iter().map(Some).collect();

        NewWorkspaceWebhook {
            workspace_id,
            display_name: self.display_name,
            description: self.description,
            url: self.url,
            secret: self.secret,
            events,
            headers: self.headers,
            status: None,
            max_failures: self.max_failures,
            created_by: account_id,
        }
    }
}

/// Request payload for updating an existing workspace webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWebhook {
    /// Updated human-readable name for the webhook (1-100 characters).
    #[validate(length(min = 1, max = 100))]
    pub display_name: Option<String>,

    /// Updated description of the webhook's purpose (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// Updated URL to send webhook payloads to.
    #[validate(url, length(min = 1, max = 2048))]
    pub url: Option<String>,

    /// Updated secret for signing webhook payloads (max 256 characters).
    #[validate(length(max = 256))]
    pub secret: Option<String>,

    /// Updated list of event types this webhook should receive.
    pub events: Option<Vec<String>>,

    /// Updated custom headers to include in webhook requests.
    pub headers: Option<serde_json::Value>,

    /// Updated maximum number of consecutive failures before disabling (1-100).
    #[validate(range(min = 1, max = 100))]
    pub max_failures: Option<i32>,
}

impl UpdateWebhook {
    /// Converts this request into an [`UpdateWorkspaceWebhookModel`].
    #[inline]
    pub fn into_model(self) -> UpdateWorkspaceWebhookModel {
        let events = self.events.map(|e| e.into_iter().map(Some).collect());

        UpdateWorkspaceWebhookModel {
            display_name: self.display_name,
            description: self.description,
            url: self.url,
            secret: self.secret.map(Some),
            events,
            headers: self.headers,
            status: None,
            max_failures: self.max_failures,
            ..Default::default()
        }
    }
}
