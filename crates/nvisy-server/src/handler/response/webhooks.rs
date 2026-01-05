//! Workspace webhook response types.

use std::collections::HashMap;

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{WebhookEvent, WebhookStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;
use crate::handler::utility::{flatten_events, parse_headers};

/// Workspace webhook response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    /// Unique webhook identifier.
    pub webhook_id: Uuid,
    /// Reference to the workspace this webhook belongs to.
    pub workspace_id: Uuid,
    /// Human-readable name for the webhook.
    pub display_name: String,
    /// Detailed description of the webhook's purpose.
    pub description: String,
    /// The URL to send webhook payloads to.
    pub url: String,
    /// List of event types this webhook receives.
    pub events: Vec<WebhookEvent>,
    /// Custom headers included in webhook requests.
    pub headers: HashMap<String, String>,
    /// Current status of the webhook.
    pub status: WebhookStatus,
    /// Timestamp of the most recent webhook trigger.
    pub last_triggered_at: Option<Timestamp>,
    /// Account that originally created this webhook.
    pub created_by: Uuid,
    /// Timestamp when this webhook was first created.
    pub created_at: Timestamp,
    /// Timestamp when this webhook was last modified.
    pub updated_at: Timestamp,
}

impl Webhook {
    pub fn from_model(webhook: model::WorkspaceWebhook) -> Self {
        Self {
            webhook_id: webhook.id,
            workspace_id: webhook.workspace_id,
            display_name: webhook.display_name,
            description: webhook.description,
            url: webhook.url,
            events: flatten_events(webhook.events),
            headers: parse_headers(webhook.headers),
            status: webhook.status,
            last_triggered_at: webhook.last_triggered_at.map(Into::into),
            created_by: webhook.created_by,
            created_at: webhook.created_at.into(),
            updated_at: webhook.updated_at.into(),
        }
    }
}

/// Paginated response for workspace webhooks.
pub type WebhooksPage = Page<Webhook>;

/// Result of a webhook delivery attempt.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResult {
    /// HTTP status code returned by the webhook endpoint.
    pub status_code: u16,
    /// Time taken to receive a response in milliseconds.
    pub response_time_ms: i64,
}

impl WebhookResult {
    /// Creates a WebhookResult from the core webhook response.
    pub fn from_response(response: nvisy_webhook::WebhookResponse) -> Self {
        let duration_ms = response
            .duration()
            .total(jiff::Unit::Millisecond)
            .unwrap_or(0.0) as i64;

        Self {
            status_code: response.status_code,
            response_time_ms: duration_ms,
        }
    }
}
