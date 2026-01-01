//! Workspace webhook response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::{WebhookEvent, WebhookStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Workspace webhook response.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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
    pub headers: serde_json::Value,

    /// Current status of the webhook.
    pub status: WebhookStatus,

    /// Timestamp of the most recent webhook trigger.
    pub last_triggered_at: Option<Timestamp>,

    /// Timestamp of the most recent successful delivery.
    pub last_success_at: Option<Timestamp>,

    /// Timestamp of the most recent failed delivery.
    pub last_failure_at: Option<Timestamp>,

    /// Account that originally created this webhook.
    pub created_by: Uuid,

    /// Timestamp when this webhook was first created.
    pub created_at: Timestamp,

    /// Timestamp when this webhook was last modified.
    pub updated_at: Timestamp,
}

/// Workspace webhook response with secret (returned only at creation).
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookWithSecret {
    /// Base webhook information.
    #[serde(flatten)]
    pub webhook: Webhook,

    /// Secret for signing webhook payloads (only shown once at creation).
    pub secret: Option<String>,
}

/// Helper function to convert Vec<Option<WebhookEvent>> to Vec<WebhookEvent>.
fn flatten_events(events: Vec<Option<WebhookEvent>>) -> Vec<WebhookEvent> {
    events.into_iter().flatten().collect()
}

impl Webhook {
    /// Creates a Webhook response from a database model.
    pub fn from_model(webhook: model::WorkspaceWebhook) -> Self {
        Self {
            webhook_id: webhook.id,
            workspace_id: webhook.workspace_id,
            display_name: webhook.display_name,
            description: webhook.description,
            url: webhook.url,
            events: flatten_events(webhook.events),
            headers: webhook.headers,
            status: webhook.status,
            last_triggered_at: webhook.last_triggered_at.map(Into::into),
            last_success_at: webhook.last_success_at.map(Into::into),
            last_failure_at: webhook.last_failure_at.map(Into::into),
            created_by: webhook.created_by,
            created_at: webhook.created_at.into(),
            updated_at: webhook.updated_at.into(),
        }
    }

    /// Creates a list of Webhook responses from database models.
    pub fn from_models(models: Vec<model::WorkspaceWebhook>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }
}

impl WebhookWithSecret {
    /// Creates a WebhookWithSecret response from a database model.
    pub fn from_model(webhook: model::WorkspaceWebhook) -> Self {
        Self {
            secret: webhook.secret.clone(),
            webhook: Webhook {
                webhook_id: webhook.id,
                workspace_id: webhook.workspace_id,
                display_name: webhook.display_name,
                description: webhook.description,
                url: webhook.url,
                events: flatten_events(webhook.events),
                headers: webhook.headers,
                status: webhook.status,
                last_triggered_at: webhook.last_triggered_at.map(Into::into),
                last_success_at: webhook.last_success_at.map(Into::into),
                last_failure_at: webhook.last_failure_at.map(Into::into),
                created_by: webhook.created_by,
                created_at: webhook.created_at.into(),
                updated_at: webhook.updated_at.into(),
            },
        }
    }
}

/// Response for listing workspace webhooks.
pub type Webhooks = Vec<Webhook>;

/// Result of a webhook test delivery attempt.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookTestResult {
    /// Whether the webhook delivery was successful (2xx status code).
    pub success: bool,

    /// HTTP status code returned by the webhook endpoint.
    pub status_code: Option<u16>,

    /// Time taken to receive a response in milliseconds.
    pub response_time_ms: Option<i64>,

    /// Error message if the delivery failed.
    pub error_message: Option<String>,
}

impl WebhookTestResult {
    /// Creates a WebhookTestResult from the core webhook response.
    pub fn from_core_response(response: nvisy_service::webhook::WebhookResponse) -> Self {
        Self {
            success: response.success,
            status_code: response.status_code,
            response_time_ms: response.response_time_ms.map(|ms| ms as i64),
            error_message: response.error,
        }
    }
}
