//! Webhook delivery request and payload types.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A webhook delivery request.
#[derive(Debug, Clone)]
pub struct WebhookRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// The webhook endpoint URL.
    pub url: String,
    /// Optional shared secret for request signing.
    pub secret: Option<String>,
    /// The JSON payload to deliver.
    pub payload: serde_json::Value,
    /// Custom headers to include in the request.
    pub headers: HashMap<String, String>,
    /// Request timeout.
    pub timeout: Duration,
}

impl WebhookRequest {
    /// Creates a new webhook request with a raw JSON payload.
    pub fn new(url: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            url: url.into(),
            secret: None,
            payload,
            headers: HashMap::new(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Creates a new webhook request from a typed payload.
    pub fn from_payload(url: impl Into<String>, payload: &WebhookPayload) -> Self {
        Self::new(url, serde_json::to_value(payload).unwrap_or_default())
    }

    /// Sets the shared secret for request signing.
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Adds a custom header to the request.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Sets multiple custom headers.
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }
}

/// The main webhook payload structure sent to webhook endpoints.
///
/// This payload is signed with HMAC-SHA256 when a webhook secret is configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WebhookPayload {
    /// Unique identifier for the webhook configuration that triggered this delivery.
    pub webhook_id: Uuid,

    /// The event type that triggered this webhook delivery.
    ///
    /// Examples: `document:created`, `workspace:updated`, `member:added`
    pub event: String,

    /// Human-readable message describing the event.
    pub message: String,

    /// Additional context about the event.
    pub context: WebhookContext,

    /// Timestamp when the event occurred (Unix timestamp in seconds).
    pub timestamp: i64,

    /// Unique identifier for this delivery attempt.
    ///
    /// Can be used for deduplication on the receiving end.
    pub delivery_id: Uuid,
}

impl WebhookPayload {
    /// Creates a new webhook payload.
    pub fn new(
        webhook_id: Uuid,
        event: impl Into<String>,
        message: impl Into<String>,
        context: WebhookContext,
    ) -> Self {
        Self {
            webhook_id,
            event: event.into(),
            message: message.into(),
            context,
            timestamp: jiff::Timestamp::now().as_second(),
            delivery_id: Uuid::now_v7(),
        }
    }

    /// Creates a test payload for webhook testing.
    pub fn test(webhook_id: Uuid) -> Self {
        Self::new(
            webhook_id,
            "webhook:test",
            "This is a test webhook delivery",
            WebhookContext::test(),
        )
    }

    /// Converts the payload into a webhook request.
    pub fn into_request(self, url: impl Into<String>) -> WebhookRequest {
        WebhookRequest::new(url, serde_json::to_value(&self).unwrap_or_default())
    }
}

/// Contextual data included with webhook payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WebhookContext {
    /// The workspace where the event occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,

    /// The primary resource affected by the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<Uuid>,

    /// The type of resource affected (e.g., "document", "member").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,

    /// The account that triggered the event (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<Uuid>,

    /// Additional event-specific metadata.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

impl WebhookContext {
    /// Creates an empty context.
    pub fn empty() -> Self {
        Self {
            workspace_id: None,
            resource_id: None,
            resource_type: None,
            actor_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Creates a test context for webhook testing.
    pub fn test() -> Self {
        Self {
            workspace_id: None,
            resource_id: None,
            resource_type: Some("test".to_string()),
            actor_id: None,
            metadata: serde_json::json!({
                "test": true,
                "message": "This is a test webhook payload"
            }),
        }
    }

    /// Sets the workspace ID.
    pub fn with_workspace(mut self, workspace_id: Uuid) -> Self {
        self.workspace_id = Some(workspace_id);
        self
    }

    /// Sets the resource information.
    pub fn with_resource(mut self, resource_id: Uuid, resource_type: impl Into<String>) -> Self {
        self.resource_id = Some(resource_id);
        self.resource_type = Some(resource_type.into());
        self
    }

    /// Sets the actor ID.
    pub fn with_actor(mut self, actor_id: Uuid) -> Self {
        self.actor_id = Some(actor_id);
        self
    }

    /// Sets additional metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

impl Default for WebhookContext {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_creation() {
        let webhook_id = Uuid::now_v7();
        let payload = WebhookPayload::new(
            webhook_id,
            "document:created",
            "A new document was created",
            WebhookContext::empty().with_resource(Uuid::now_v7(), "document"),
        );

        assert_eq!(payload.webhook_id, webhook_id);
        assert_eq!(payload.event, "document:created");
        assert_eq!(payload.message, "A new document was created");
        assert!(payload.context.resource_type.is_some());
    }

    #[test]
    fn test_payload_into_request() {
        let payload = WebhookPayload::test(Uuid::now_v7());
        let request = payload.into_request("https://example.com/webhook");

        assert_eq!(request.url, "https://example.com/webhook");
        assert!(request.payload.get("event").is_some());
    }

    #[test]
    fn test_request_from_payload() {
        let payload = WebhookPayload::test(Uuid::now_v7());
        let request = WebhookRequest::from_payload("https://example.com/webhook", &payload);

        assert_eq!(request.url, "https://example.com/webhook");
        assert!(request.payload.get("webhook_id").is_some());
    }

    #[test]
    fn test_context_builder() {
        let workspace_id = Uuid::now_v7();
        let resource_id = Uuid::now_v7();
        let actor_id = Uuid::now_v7();

        let context = WebhookContext::empty()
            .with_workspace(workspace_id)
            .with_resource(resource_id, "document")
            .with_actor(actor_id)
            .with_metadata(serde_json::json!({"key": "value"}));

        assert_eq!(context.workspace_id, Some(workspace_id));
        assert_eq!(context.resource_id, Some(resource_id));
        assert_eq!(context.resource_type, Some("document".to_string()));
        assert_eq!(context.actor_id, Some(actor_id));
        assert!(context.metadata.get("key").is_some());
    }
}
