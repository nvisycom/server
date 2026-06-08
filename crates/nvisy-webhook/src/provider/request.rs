//! Webhook delivery request and payload types.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::WebhookContext;

/// A webhook delivery request.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct WebhookRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// The webhook endpoint URL.
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<u64>"))]
    pub timeout: Option<Duration>,
    /// HMAC-SHA256 signing secret for request authentication.
    #[serde(default, skip_serializing)]
    #[cfg_attr(feature = "schema", schemars(skip))]
    pub secret: Option<String>,
}

impl fmt::Debug for WebhookRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookRequest")
            .field("request_id", &self.request_id)
            .field("url", &self.url)
            .field("event", &self.event)
            .field("message", &self.message)
            .field("context", &self.context)
            .field("headers", &self.headers)
            .field("timeout", &self.timeout)
            .field("secret", &self.secret.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
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
            secret: None,
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

    /// Sets the signing secret for HMAC-SHA256 authentication.
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
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
}
