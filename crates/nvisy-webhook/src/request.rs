//! Webhook delivery request and payload types.

use std::collections::HashMap;
use std::time::Duration;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

/// A webhook delivery request.
#[derive(Debug, Clone)]
pub struct WebhookRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// The webhook endpoint URL.
    pub url: Url,
    /// The event type that triggered this webhook delivery.
    pub event: String,
    /// Human-readable message describing the event.
    pub message: String,
    /// Additional context about the event.
    pub context: WebhookContext,
    /// Custom headers to include in the request.
    pub headers: HashMap<String, String>,
    /// Optional request timeout (uses client default if not set).
    pub timeout: Option<Duration>,
}

impl WebhookRequest {
    /// Creates a new webhook request.
    pub fn new(
        url: Url,
        event: impl Into<String>,
        message: impl Into<String>,
        context: WebhookContext,
    ) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            url,
            event: event.into(),
            message: message.into(),
            context,
            headers: HashMap::new(),
            timeout: None,
        }
    }

    /// Creates a test request for webhook testing.
    pub fn test(url: Url, webhook_id: Uuid, workspace_id: Uuid) -> Self {
        Self::new(
            url,
            "webhook:test",
            "This is a test webhook delivery",
            WebhookContext::test(webhook_id, workspace_id),
        )
    }

    /// Sets the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
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

    /// Converts this request into a payload for serialization.
    pub fn into_payload(self) -> WebhookPayload {
        WebhookPayload {
            event: self.event,
            message: self.message,
            context: self.context,
            timestamp: Timestamp::now(),
        }
    }

    /// Creates a payload from this request without consuming it.
    pub fn to_payload(&self) -> WebhookPayload {
        WebhookPayload {
            event: self.event.clone(),
            message: self.message.clone(),
            context: self.context.clone(),
            timestamp: Timestamp::now(),
        }
    }
}

/// The webhook payload structure sent to webhook endpoints.
///
/// This payload is signed with HMAC-SHA256 when a webhook secret is configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WebhookPayload {
    /// The event type that triggered this webhook delivery.
    pub event: String,

    /// Human-readable message describing the event.
    pub message: String,

    /// Additional context about the event.
    pub context: WebhookContext,

    /// Timestamp when the payload was created.
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub timestamp: Timestamp,
}

/// Contextual data included with webhook payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WebhookContext {
    /// Unique identifier for the webhook configuration that triggered this delivery.
    pub webhook_id: Uuid,

    /// The workspace where the event occurred.
    pub workspace_id: Uuid,

    /// The primary resource affected by the event.
    pub resource_id: Uuid,

    /// The type of resource affected (e.g., "document", "member").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,

    /// The account that triggered the event (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,

    /// Additional event-specific metadata.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

impl WebhookContext {
    /// Creates a new context with required fields.
    pub fn new(webhook_id: Uuid, workspace_id: Uuid, resource_id: Uuid) -> Self {
        Self {
            webhook_id,
            workspace_id,
            resource_id,
            resource_type: None,
            account_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Creates a test context for webhook testing.
    pub fn test(webhook_id: Uuid, workspace_id: Uuid) -> Self {
        Self {
            webhook_id,
            workspace_id,
            resource_id: webhook_id, // Use webhook_id as resource_id for tests
            resource_type: Some("webhook".to_string()),
            account_id: None,
            metadata: serde_json::json!({
                "test": true
            }),
        }
    }

    /// Sets the resource type.
    pub fn with_resource_type(mut self, resource_type: impl Into<String>) -> Self {
        self.resource_type = Some(resource_type.into());
        self
    }

    /// Sets the account ID.
    pub fn with_account(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Sets additional metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_creation() {
        let webhook_id = Uuid::now_v7();
        let workspace_id = Uuid::now_v7();
        let resource_id = Uuid::now_v7();
        let url = Url::parse("https://example.com/webhook").unwrap();

        let context = WebhookContext::new(webhook_id, workspace_id, resource_id);
        let request = WebhookRequest::new(
            url.clone(),
            "document:created",
            "A new document was created",
            context,
        );

        assert_eq!(request.url, url);
        assert_eq!(request.event, "document:created");
        assert_eq!(request.context.webhook_id, webhook_id);
        assert!(request.timeout.is_none());
    }

    #[test]
    fn test_request_to_payload() {
        let webhook_id = Uuid::now_v7();
        let workspace_id = Uuid::now_v7();
        let url = Url::parse("https://example.com/webhook").unwrap();

        let request = WebhookRequest::test(url, webhook_id, workspace_id);
        let payload = request.to_payload();

        assert_eq!(payload.event, "webhook:test");
        assert_eq!(payload.context.webhook_id, webhook_id);
        assert_eq!(payload.context.workspace_id, workspace_id);
    }

    #[test]
    fn test_context_builder() {
        let webhook_id = Uuid::now_v7();
        let workspace_id = Uuid::now_v7();
        let resource_id = Uuid::now_v7();
        let account_id = Uuid::now_v7();

        let context = WebhookContext::new(webhook_id, workspace_id, resource_id)
            .with_resource_type("document")
            .with_account(account_id)
            .with_metadata(serde_json::json!({"key": "value"}));

        assert_eq!(context.webhook_id, webhook_id);
        assert_eq!(context.workspace_id, workspace_id);
        assert_eq!(context.resource_id, resource_id);
        assert_eq!(context.resource_type, Some("document".to_string()));
        assert_eq!(context.account_id, Some(account_id));
        assert!(context.metadata.get("key").is_some());
    }
}
