//! Workspace webhook model for PostgreSQL database operations.
//!
//! This module provides models for managing webhooks connected to workspaces.
//! Webhooks enable workspaces to send event notifications to external services.

use std::collections::HashMap;

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_webhooks;
use crate::types::{
    HasCreatedAt, HasDeletedAt, HasOwnership, HasUpdatedAt, WebhookEvent, WebhookStatus,
};

/// Workspace webhook model representing a webhook configuration for a workspace.
///
/// This model manages webhook endpoints that receive event notifications from
/// workspaces. Each webhook maintains its own lifecycle with status tracking
/// and delivery monitoring.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceWebhook {
    /// Unique webhook identifier.
    pub id: Uuid,
    /// Reference to the workspace this webhook belongs to.
    pub workspace_id: Uuid,
    /// Human-readable name for the webhook.
    pub display_name: String,
    /// Description of the webhook's purpose.
    pub description: String,
    /// Webhook endpoint URL.
    pub url: String,
    /// Array of event types this webhook subscribes to.
    pub events: Vec<Option<WebhookEvent>>,
    /// Custom headers to include in webhook requests.
    pub headers: serde_json::Value,
    /// HMAC-SHA256 signing secret for webhook verification.
    pub secret: String,
    /// Current status of the webhook.
    pub status: WebhookStatus,
    /// Timestamp of last webhook trigger.
    pub last_triggered_at: Option<Timestamp>,
    /// Account that created this webhook.
    pub created_by: Uuid,
    /// Timestamp when this webhook was created.
    pub created_at: Timestamp,
    /// Timestamp when this webhook was last modified.
    pub updated_at: Timestamp,
    /// Timestamp when this webhook was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data structure for creating a new workspace webhook.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceWebhook {
    /// Reference to the workspace this webhook will belong to.
    pub workspace_id: Uuid,
    /// Human-readable name for the webhook.
    pub display_name: String,
    /// Description of the webhook's purpose.
    pub description: String,
    /// Webhook endpoint URL.
    pub url: String,
    /// Array of event types this webhook subscribes to.
    pub events: Vec<Option<WebhookEvent>>,
    /// Custom headers to include in webhook requests.
    pub headers: Option<serde_json::Value>,
    /// Initial status of the webhook.
    pub status: Option<WebhookStatus>,
    /// Account creating this webhook.
    pub created_by: Uuid,
}

/// Data structure for updating an existing workspace webhook.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceWebhook {
    /// Updated name for the webhook.
    pub display_name: Option<String>,
    /// Updated description.
    pub description: Option<String>,
    /// Updated endpoint URL.
    pub url: Option<String>,
    /// Updated event subscriptions.
    pub events: Option<Vec<Option<WebhookEvent>>>,
    /// Updated custom headers.
    pub headers: Option<serde_json::Value>,
    /// Updated status.
    pub status: Option<WebhookStatus>,
    /// Updated last triggered timestamp.
    pub last_triggered_at: Option<Option<Timestamp>>,
    /// Soft deletion timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl WorkspaceWebhook {
    /// Returns whether the webhook is active and receiving events.
    pub fn is_active(&self) -> bool {
        self.status.is_active() && self.deleted_at.is_none()
    }

    /// Returns whether the webhook is currently paused.
    pub fn is_paused(&self) -> bool {
        self.status.is_paused()
    }

    /// Returns whether the webhook is disabled.
    pub fn is_disabled(&self) -> bool {
        self.status.is_disabled()
    }

    /// Returns whether the webhook has custom headers.
    pub fn has_custom_headers(&self) -> bool {
        !self.headers.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns the list of subscribed events.
    pub fn subscribed_events(&self) -> Vec<WebhookEvent> {
        self.events.iter().filter_map(|e| *e).collect()
    }

    /// Returns the custom headers as a `HashMap<String, String>`.
    pub fn parsed_headers(&self) -> HashMap<String, String> {
        serde_json::from_value(self.headers.clone()).unwrap_or_default()
    }

    /// Returns whether the webhook subscribes to a specific event type.
    pub fn subscribes_to(&self, event: WebhookEvent) -> bool {
        self.events.contains(&Some(event))
    }

    /// Returns whether the webhook has been triggered at least once.
    pub fn has_been_triggered(&self) -> bool {
        self.last_triggered_at.is_some()
    }

    /// Returns whether the webhook is in a healthy state.
    pub fn is_healthy(&self) -> bool {
        self.is_active()
    }
}

impl HasCreatedAt for WorkspaceWebhook {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceWebhook {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for WorkspaceWebhook {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}

impl HasOwnership for WorkspaceWebhook {
    fn created_by(&self) -> Uuid {
        self.created_by
    }
}

impl NewWorkspaceWebhook {
    /// Converts a `HashMap<String, String>` to `Option<serde_json::Value>`.
    ///
    /// Returns `None` if the map is empty.
    pub fn serialize_headers(headers: HashMap<String, String>) -> Option<serde_json::Value> {
        if headers.is_empty() {
            None
        } else {
            Some(serde_json::to_value(&headers).unwrap_or_default())
        }
    }

    /// Converts an `Option<HashMap<String, String>>` to `Option<serde_json::Value>`.
    ///
    /// Returns `None` if the input is `None` or the map is empty.
    pub fn serialize_headers_opt(
        headers: Option<HashMap<String, String>>,
    ) -> Option<serde_json::Value> {
        headers.and_then(Self::serialize_headers)
    }
}
