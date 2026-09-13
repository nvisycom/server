//! Workspace webhook request types.
//!
//! This module provides request DTOs for workspace webhook management including
//! creation and updates.

use std::collections::HashMap;

use garde::Validate;
use nvisy_postgres::types::{WebhookEvent, WebhookStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::input::{CreateWebhookInput, UpdateWebhookInput};

/// Request payload for creating a new workspace webhook.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateWorkspaceWebhook {
    /// Human-readable name for the webhook (1-128 characters).
    #[garde(length(chars, min = 1, max = 128))]
    pub display_name: String,
    /// Detailed description of the webhook's purpose (max 500 characters).
    #[garde(length(chars, max = 500))]
    pub description: String,
    /// The URL to send webhook payloads to.
    #[garde(url, length(chars, max = 2048))]
    pub url: String,
    /// List of event types this webhook should receive.
    pub events: Vec<WebhookEvent>,
    /// Optional custom headers to include in webhook requests.
    pub headers: Option<HashMap<String, String>>,
    /// Initial status of the webhook (enabled or disabled).
    pub status: Option<WebhookStatus>,
}

impl From<CreateWorkspaceWebhook> for CreateWebhookInput {
    fn from(request: CreateWorkspaceWebhook) -> Self {
        CreateWebhookInput {
            display_name: request.display_name,
            description: request.description,
            url: request.url,
            events: request.events,
            headers: request.headers,
            status: request.status,
        }
    }
}

/// Request payload for updating an existing workspace webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct UpdateWorkspaceWebhook {
    /// Updated human-readable name for the webhook (1-128 characters).
    #[garde(length(chars, min = 1, max = 128))]
    pub display_name: Option<String>,
    /// Updated description of the webhook's purpose (max 500 characters).
    #[garde(length(chars, max = 500))]
    pub description: Option<String>,
    /// Updated URL to send webhook payloads to.
    #[garde(url, length(chars, max = 2048))]
    pub url: Option<String>,
    /// Updated list of event types this webhook should receive.
    pub events: Option<Vec<WebhookEvent>>,
    /// Updated custom headers to include in webhook requests.
    pub headers: Option<HashMap<String, String>>,
    /// Updated status (enabled or disabled). Ignored while the webhook is
    /// system-suspended.
    pub status: Option<WebhookStatus>,
}

impl From<UpdateWorkspaceWebhook> for UpdateWebhookInput {
    fn from(request: UpdateWorkspaceWebhook) -> Self {
        UpdateWebhookInput {
            display_name: request.display_name,
            description: request.description,
            url: request.url,
            events: request.events,
            headers: request.headers,
            status: request.status,
        }
    }
}

/// Request payload for testing a webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct TestWorkspaceWebhook {
    /// Optional custom payload to send in the test request.
    /// If not provided, a default test payload will be used.
    pub payload: Option<serde_json::Value>,
}
