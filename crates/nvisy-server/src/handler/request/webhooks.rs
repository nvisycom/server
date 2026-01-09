//! Workspace webhook request types.
//!
//! This module provides request DTOs for workspace webhook management including
//! creation and updates.

use std::collections::HashMap;

use nvisy_postgres::model::{
    NewWorkspaceWebhook, UpdateWorkspaceWebhook as UpdateWorkspaceWebhookModel,
};
use nvisy_postgres::types::{WebhookEvent, WebhookStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::handler::utility::serialize_headers_opt;

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
    pub description: String,
    /// The URL to send webhook payloads to.
    #[validate(url, length(min = 1, max = 2048))]
    pub url: String,
    /// List of event types this webhook should receive.
    pub events: Vec<WebhookEvent>,
    /// Optional custom headers to include in webhook requests.
    pub headers: Option<HashMap<String, String>>,
    /// Initial status of the webhook (active or paused).
    pub status: Option<WebhookStatus>,
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
        let headers = serialize_headers_opt(self.headers);
        // Treat Disabled as Paused since users cannot set Disabled status
        let status = self.status.map(|s| match s {
            WebhookStatus::Disabled => WebhookStatus::Paused,
            other => other,
        });

        NewWorkspaceWebhook {
            workspace_id,
            display_name: self.display_name,
            description: self.description,
            url: self.url,
            events,
            headers,
            status,
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
    /// Updated list of event types this webhook should receive.
    pub events: Option<Vec<WebhookEvent>>,
    /// Updated custom headers to include in webhook requests.
    pub headers: Option<HashMap<String, String>>,
    /// Updated status (active or paused). Ignored if webhook is currently disabled.
    pub status: Option<WebhookStatus>,
}

impl UpdateWebhook {
    /// Converts this request into an [`UpdateWorkspaceWebhookModel`].
    ///
    /// If `current_status` is `Disabled`, the status field is ignored.
    /// If user tries to set `Disabled`, it's treated as `Paused`.
    #[inline]
    pub fn into_model(self, current_status: WebhookStatus) -> UpdateWorkspaceWebhookModel {
        let events = self.events.map(|e| e.into_iter().map(Some).collect());
        let headers = serialize_headers_opt(self.headers);
        // Ignore status changes if webhook is disabled; treat Disabled as Paused
        let status = if current_status.is_disabled() {
            None
        } else {
            self.status.map(|s| match s {
                WebhookStatus::Disabled => WebhookStatus::Paused,
                other => other,
            })
        };

        UpdateWorkspaceWebhookModel {
            display_name: self.display_name,
            description: self.description,
            url: self.url,
            events,
            headers,
            status,
            ..Default::default()
        }
    }
}

/// Request payload for testing a webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct TestWebhook {
    /// Optional custom payload to send in the test request.
    /// If not provided, a default test payload will be used.
    pub payload: Option<serde_json::Value>,
}
