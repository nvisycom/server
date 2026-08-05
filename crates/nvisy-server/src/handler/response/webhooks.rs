//! Workspace webhook response types.

use std::collections::HashMap;

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{Handle, WebhookEvent, WebhookId, WebhookStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AccountRef, Page};

/// Workspace webhook response.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    /// Opaque identifier of the webhook.
    pub id: WebhookId,
    /// Handle of the workspace this webhook belongs to.
    pub workspace_slug: Handle,
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
    /// Timestamp of the most recent successful delivery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<Timestamp>,
    /// Timestamp of the most recent failed delivery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_at: Option<Timestamp>,
    /// Consecutive failed deliveries since the last success.
    pub consecutive_failures: i32,
    /// Account that created this webhook.
    pub created_by: AccountRef,
    /// Timestamp when this webhook was first created.
    pub created_at: Timestamp,
    /// Timestamp when this webhook was last modified.
    pub updated_at: Timestamp,
}

impl Webhook {
    pub fn from_model(
        webhook: model::WorkspaceWebhook,
        workspace_slug: Handle,
        created_by: AccountRef,
    ) -> Self {
        let events = webhook.subscribed_events();
        let headers = webhook.parsed_headers();

        Self {
            id: WebhookId::from_uuid(webhook.id),
            workspace_slug,
            display_name: webhook.display_name,
            description: webhook.description,
            url: webhook.url,
            events,
            headers,
            status: webhook.status,
            last_success_at: webhook.last_success_at.map(Into::into),
            last_failure_at: webhook.last_failure_at.map(Into::into),
            consecutive_failures: webhook.consecutive_failures,
            created_by,
            created_at: webhook.created_at.into(),
            updated_at: webhook.updated_at.into(),
        }
    }
}

/// Webhook creation response that includes the secret (visible only once).
///
/// The secret is used for HMAC-SHA256 signature verification of webhook payloads.
/// It is only returned when the webhook is first created and cannot be retrieved
/// again. Store it securely.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookCreated {
    /// The created webhook details.
    #[serde(flatten)]
    pub webhook: Webhook,
    /// HMAC-SHA256 signing secret for webhook verification.
    ///
    /// **Important**: This is the only time the secret will be shown.
    /// Store it securely as it cannot be retrieved again.
    pub secret: String,
}

impl WebhookCreated {
    pub fn from_model(
        webhook: model::WorkspaceWebhook,
        workspace_slug: Handle,
        created_by: AccountRef,
        secret: String,
    ) -> Self {
        Self {
            webhook: Webhook::from_model(webhook, workspace_slug, created_by),
            secret,
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
    pub fn from_response(response: nvisy_webhook::provider::WebhookResponse) -> Self {
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
