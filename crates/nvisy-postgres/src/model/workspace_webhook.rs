//! Workspace webhook model for PostgreSQL database operations.
//!
//! This module provides models for managing webhooks connected to workspaces.
//! Webhooks enable workspaces to send event notifications to external services.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_webhooks;
use crate::types::{Json, WebhookEvent, WebhookHeaders, WebhookStatus};

/// Workspace webhook model representing a webhook configuration for a workspace.
///
/// This model manages webhook endpoints that receive event notifications from
/// workspaces. Each webhook maintains its own lifecycle with status tracking
/// and delivery monitoring.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
    pub headers: Json<WebhookHeaders>,
    /// HMAC-SHA256 signing secret, encrypted under the workspace key.
    pub encrypted_secret: Vec<u8>,
    /// Current status of the webhook.
    pub status: WebhookStatus,
    /// Timestamp of the last successful delivery.
    pub last_success_at: Option<Timestamp>,
    /// Timestamp of the last failed delivery.
    pub last_failure_at: Option<Timestamp>,
    /// Consecutive failed deliveries; reset on success, drives auto-disable.
    pub consecutive_failures: i32,
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
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
    pub headers: Option<Json<WebhookHeaders>>,
    /// HMAC-SHA256 signing secret, encrypted under the workspace key.
    pub encrypted_secret: Vec<u8>,
    /// Initial status of the webhook.
    pub status: Option<WebhookStatus>,
    /// Account creating this webhook.
    pub created_by: Uuid,
}

/// Data structure for updating an existing workspace webhook.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_webhooks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
    pub headers: Option<Json<WebhookHeaders>>,
    /// Updated status.
    pub status: Option<WebhookStatus>,
    /// Soft deletion timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl NewWorkspaceWebhook {
    /// A minimal enabled webhook for `workspace_id`, subscribed to `events`, for
    /// tests. The URL is a valid `https://` endpoint and the status takes its
    /// `enabled` database default.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, created_by: Uuid, events: Vec<WebhookEvent>) -> Self {
        Self {
            workspace_id,
            display_name: "Test Webhook".to_owned(),
            description: String::new(),
            url: "https://example.com/hook".to_owned(),
            events: events.into_iter().map(Some).collect(),
            headers: None,
            encrypted_secret: vec![1, 2, 3],
            status: None,
            created_by,
        }
    }
}

impl WorkspaceWebhook {
    /// Returns the list of subscribed events.
    pub fn subscribed_events(&self) -> Vec<WebhookEvent> {
        self.events.iter().filter_map(|e| *e).collect()
    }

    /// Returns the custom headers, or an empty set for an absent/older blob.
    pub fn parsed_headers(&self) -> WebhookHeaders {
        self.headers.or_default()
    }
}
