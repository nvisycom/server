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
pub struct ProjectWebhook {
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

/// Project webhook response with secret (for sensitive operations).
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWebhookWithSecret {
    /// Base webhook information.
    #[serde(flatten)]
    pub webhook: ProjectWebhook,

    /// Secret for signing webhook payloads.
    pub secret: Option<String>,
}

/// Summary information about a project webhook for list views.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWebhookSummary {
    /// Unique webhook identifier.
    pub webhook_id: Uuid,

    /// Reference to the project this webhook belongs to.
    pub project_id: Uuid,

    /// Human-readable name for the webhook.
    pub display_name: String,

    /// The URL to send webhook payloads to.
    pub url: String,

    /// List of event types this webhook receives.
    pub events: Vec<String>,

    /// Current status of the webhook.
    pub status: WebhookStatus,

    /// Number of consecutive delivery failures.
    pub failure_count: i32,

    /// Timestamp of the most recent webhook trigger.
    pub last_triggered_at: Option<Timestamp>,

    /// Timestamp when this webhook was first created.
    pub created_at: Timestamp,
}

/// Helper function to convert Vec<Option<String>> to Vec<String>.
fn flatten_events(events: Vec<Option<String>>) -> Vec<String> {
    events.into_iter().flatten().collect()
}

impl ProjectWebhook {
    /// Creates a new instance of [`ProjectWebhook`] from database model.
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

impl ProjectWebhookWithSecret {
    /// Creates a new instance of [`ProjectWebhookWithSecret`] from database model.
    pub fn from_model(webhook: model::ProjectWebhook) -> Self {
        Self {
            secret: webhook.secret.clone(),
            webhook: ProjectWebhook::new(&webhook),
        }
    }
}

impl ProjectWebhookSummary {
    /// Creates a new instance of [`ProjectWebhookSummary`] from database model.
    pub fn from_model(webhook: model::ProjectWebhook) -> Self {
        Self {
            webhook_id: webhook.id,
            project_id: webhook.project_id,
            display_name: webhook.display_name,
            url: webhook.url,
            events: flatten_events(webhook.events),
            status: webhook.status,
            failure_count: webhook.failure_count,
            last_triggered_at: webhook.last_triggered_at.map(Into::into),
            created_at: webhook.created_at.into(),
        }
    }
}

impl From<model::ProjectWebhook> for ProjectWebhook {
    #[inline]
    fn from(webhook: model::ProjectWebhook) -> Self {
        Self::new(&webhook)
    }
}

impl From<model::ProjectWebhook> for ProjectWebhookWithSecret {
    #[inline]
    fn from(webhook: model::ProjectWebhook) -> Self {
        Self::from_model(webhook)
    }
}

impl From<model::ProjectWebhook> for ProjectWebhookSummary {
    #[inline]
    fn from(webhook: model::ProjectWebhook) -> Self {
        Self::from_model(webhook)
    }
}

/// Response for listing project webhooks.
pub type ProjectWebhooks = Vec<ProjectWebhook>;

/// Response for listing project webhook summaries.
pub type ProjectWebhookSummaries = Vec<ProjectWebhookSummary>;
