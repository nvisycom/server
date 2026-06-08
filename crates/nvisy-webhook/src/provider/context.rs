//! Contextual data included with webhook payloads.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
