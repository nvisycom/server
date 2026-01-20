//! Output node configuration types.

use serde::{Deserialize, Serialize};

/// Output node configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputConfig {
    /// Storage backend output (S3, GCS, Azure, etc.).
    Storage(nvisy_opendal::StorageConfig),
    /// Send to webhook.
    Webhook(WebhookConfig),
}

/// Webhook output configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL.
    pub url: String,
    /// HTTP method.
    #[serde(default = "default_post")]
    pub method: String,
    /// Additional headers.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_post() -> String {
    "POST".to_string()
}
