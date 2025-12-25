//! Project webhook request types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request payload for creating a new project webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebhook {
    /// Human-readable name for the webhook.
    #[validate(length(min = 1, max = 100))]
    pub display_name: String,

    /// Detailed description of the webhook's purpose.
    #[validate(length(max = 500))]
    #[serde(default)]
    pub description: String,

    /// The URL to send webhook payloads to.
    #[validate(url, length(min = 1, max = 2048))]
    pub url: String,

    /// Optional secret for signing webhook payloads.
    #[validate(length(max = 256))]
    pub secret: Option<String>,

    /// List of event types this webhook should receive.
    pub events: Vec<String>,

    /// Optional custom headers to include in webhook requests.
    pub headers: Option<serde_json::Value>,

    /// Maximum number of consecutive failures before disabling.
    #[validate(range(min = 1, max = 100))]
    pub max_failures: Option<i32>,
}

/// Request payload for updating an existing project webhook.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWebhook {
    /// Updated human-readable name for the webhook.
    #[validate(length(min = 1, max = 100))]
    pub display_name: Option<String>,

    /// Updated description of the webhook's purpose.
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// Updated URL to send webhook payloads to.
    #[validate(url, length(min = 1, max = 2048))]
    pub url: Option<String>,

    /// Updated secret for signing webhook payloads.
    #[validate(length(max = 256))]
    pub secret: Option<String>,

    /// Updated list of event types this webhook should receive.
    pub events: Option<Vec<String>>,

    /// Updated custom headers to include in webhook requests.
    pub headers: Option<serde_json::Value>,

    /// Updated maximum number of consecutive failures before disabling.
    #[validate(range(min = 1, max = 100))]
    pub max_failures: Option<i32>,
}
