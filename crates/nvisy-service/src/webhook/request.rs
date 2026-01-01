//! Webhook delivery request and payload types.

use std::collections::HashMap;
use std::time::Duration;

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
    /// The webhook payload to deliver.
    pub payload: WebhookPayload,
    /// Custom headers to include in the request.
    pub headers: HashMap<String, String>,
    /// Optional request timeout (uses client default if not set).
    pub timeout: Option<Duration>,
}

impl WebhookRequest {
    /// Creates a new webhook request.
    pub fn new(url: Url, payload: WebhookPayload) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            url,
            payload,
            headers: HashMap::new(),
            timeout: None,
        }
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
}

/// The main webhook payload structure sent to webhook endpoints.
///
/// This payload is signed with HMAC-SHA256 when a webhook secret is configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WebhookPayload {
    /// The event type that triggered this webhook delivery.
    ///
    /// Examples: `document:created`, `workspace:updated`, `member:added`
    pub event: String,

    /// Human-readable message describing the event.
    pub message: String,

    /// Additional context about the event.
    pub context: WebhookContext,

    /// Timestamp when the event occurred.
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub timestamp: jiff::Timestamp,
}

impl WebhookPayload {
    /// Creates a new webhook payload.
    pub fn new(
        event: impl Into<String>,
        message: impl Into<String>,
        context: WebhookContext,
    ) -> Self {
        Self {
            event: event.into(),
            message: message.into(),
            context,
            timestamp: jiff::Timestamp::now(),
        }
    }

    /// Creates a test payload for webhook testing.
    pub fn test(webhook_id: Uuid) -> Self {
        Self::new(
            "webhook:test",
            "This is a test webhook delivery",
            WebhookContext::test(webhook_id),
        )
    }

    /// Converts the payload into a webhook request.
    pub fn into_request(self, url: Url) -> WebhookRequest {
        WebhookRequest::new(url, self)
    }
}

/// Contextual data included with webhook payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WebhookContext {
    /// Unique identifier for the webhook configuration that triggered this delivery.
    pub webhook_id: Uuid,

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
    pub account_id: Option<Uuid>,

    /// Additional event-specific metadata.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

impl WebhookContext {
    /// Creates a new context with only the webhook ID.
    pub fn new(webhook_id: Uuid) -> Self {
        Self {
            webhook_id,
            workspace_id: None,
            resource_id: None,
            resource_type: None,
            account_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Creates a test context for webhook testing.
    pub fn test(webhook_id: Uuid) -> Self {
        Self {
            webhook_id,
            workspace_id: None,
            resource_id: None,
            resource_type: Some("test".to_string()),
            account_id: None,
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
    fn test_payload_creation() {
        let webhook_id = Uuid::now_v7();
        let payload = WebhookPayload::new(
            "document:created",
            "A new document was created",
            WebhookContext::new(webhook_id).with_resource(Uuid::now_v7(), "document"),
        );

        assert_eq!(payload.event, "document:created");
        assert_eq!(payload.message, "A new document was created");
        assert_eq!(payload.context.webhook_id, webhook_id);
        assert!(payload.context.resource_type.is_some());
    }

    #[test]
    fn test_payload_into_request() {
        let webhook_id = Uuid::now_v7();
        let payload = WebhookPayload::test(webhook_id);
        let url = Url::parse("https://example.com/webhook").unwrap();
        let request = payload.into_request(url.clone());

        assert_eq!(request.url, url);
        assert_eq!(request.payload.event, "webhook:test");
        assert_eq!(request.payload.context.webhook_id, webhook_id);
    }

    #[test]
    fn test_request_creation() {
        let webhook_id = Uuid::now_v7();
        let payload = WebhookPayload::test(webhook_id);
        let url = Url::parse("https://example.com/webhook").unwrap();
        let request = WebhookRequest::new(url.clone(), payload);

        assert_eq!(request.url, url);
        assert!(request.timeout.is_none());
    }

    #[test]
    fn test_context_builder() {
        let webhook_id = Uuid::now_v7();
        let workspace_id = Uuid::now_v7();
        let resource_id = Uuid::now_v7();
        let account_id = Uuid::now_v7();

        let context = WebhookContext::new(webhook_id)
            .with_workspace(workspace_id)
            .with_resource(resource_id, "document")
            .with_account(account_id)
            .with_metadata(serde_json::json!({"key": "value"}));

        assert_eq!(context.webhook_id, webhook_id);
        assert_eq!(context.workspace_id, Some(workspace_id));
        assert_eq!(context.resource_id, Some(resource_id));
        assert_eq!(context.resource_type, Some("document".to_string()));
        assert_eq!(context.account_id, Some(account_id));
        assert!(context.metadata.get("key").is_some());
    }
}
