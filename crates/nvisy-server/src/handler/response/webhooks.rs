//! Project webhook response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::WebhookStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Project webhook response.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    /// Unique webhook identifier.
    pub webhook_id: Uuid,

    /// Reference to the project this webhook belongs to.
    pub project_id: Uuid,

    /// Human-readable name for the webhook.
    pub display_name: String,

    /// Detailed description of the webhook's purpose.
    pub description: String,

    /// The URL to send webhook payloads to.
    pub url: String,

    /// List of event types this webhook receives.
    pub events: Vec<String>,

    /// Custom headers included in webhook requests.
    pub headers: serde_json::Value,

    /// Current status of the webhook.
    pub status: WebhookStatus,

    /// Number of consecutive delivery failures.
    pub failure_count: i32,

    /// Maximum failures before automatic disabling.
    pub max_failures: i32,

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

/// Project webhook response with secret (returned only at creation).
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

/// Helper function to convert Vec<Option<String>> to Vec<String>.
fn flatten_events(events: Vec<Option<String>>) -> Vec<String> {
    events.into_iter().flatten().collect()
}

impl Webhook {
    /// Creates a new instance of [`Webhook`] from database model.
    pub fn new(webhook: &model::ProjectWebhook) -> Self {
        Self {
            webhook_id: webhook.id,
            project_id: webhook.project_id,
            display_name: webhook.display_name.clone(),
            description: webhook.description.clone(),
            url: webhook.url.clone(),
            events: flatten_events(webhook.events.clone()),
            headers: webhook.headers.clone(),
            status: webhook.status,
            failure_count: webhook.failure_count,
            max_failures: webhook.max_failures,
            last_triggered_at: webhook.last_triggered_at.map(Into::into),
            last_success_at: webhook.last_success_at.map(Into::into),
            last_failure_at: webhook.last_failure_at.map(Into::into),
            created_by: webhook.created_by,
            created_at: webhook.created_at.into(),
            updated_at: webhook.updated_at.into(),
        }
    }
}

impl WebhookWithSecret {
    /// Creates a new instance of [`WebhookWithSecret`] from database model.
    pub fn from_model(webhook: model::ProjectWebhook) -> Self {
        Self {
            secret: webhook.secret.clone(),
            webhook: Webhook::new(&webhook),
        }
    }
}

impl From<model::ProjectWebhook> for Webhook {
    #[inline]
    fn from(webhook: model::ProjectWebhook) -> Self {
        Self::new(&webhook)
    }
}

impl From<model::ProjectWebhook> for WebhookWithSecret {
    #[inline]
    fn from(webhook: model::ProjectWebhook) -> Self {
        Self::from_model(webhook)
    }
}

/// Response for listing project webhooks.
pub type Webhooks = Vec<Webhook>;
