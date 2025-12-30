//! Workspace webhook model for PostgreSQL database operations.
//!
//! This module provides models for managing webhooks connected to workspaces.
//! Webhooks enable workspaces to send event notifications to external services.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_webhooks;
use crate::types::{HasCreatedAt, HasDeletedAt, HasOwnership, HasUpdatedAt, WebhookStatus};

/// Workspace webhook model representing a webhook configuration for a workspace.
///
/// This model manages webhook endpoints that receive event notifications from
/// workspaces. Each webhook maintains its own lifecycle with status tracking,
/// failure handling, and delivery monitoring.
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

    /// Shared secret for signature verification.
    pub secret: Option<String>,

    /// Array of event types this webhook subscribes to.
    pub events: Vec<Option<String>>,

    /// Custom headers to include in webhook requests.
    pub headers: serde_json::Value,

    /// Current status of the webhook.
    pub status: WebhookStatus,

    /// Consecutive failure count.
    pub failure_count: i32,

    /// Maximum failures before auto-disable.
    pub max_failures: i32,

    /// Timestamp of last webhook trigger.
    pub last_triggered_at: Option<Timestamp>,

    /// Timestamp of last successful delivery.
    pub last_success_at: Option<Timestamp>,

    /// Timestamp of last failed delivery.
    pub last_failure_at: Option<Timestamp>,

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

    /// Shared secret for signature verification.
    pub secret: Option<String>,

    /// Array of event types this webhook subscribes to.
    pub events: Vec<Option<String>>,

    /// Custom headers to include in webhook requests.
    pub headers: Option<serde_json::Value>,

    /// Initial status of the webhook.
    pub status: Option<WebhookStatus>,

    /// Maximum failures before auto-disable.
    pub max_failures: Option<i32>,

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

    /// Updated shared secret.
    pub secret: Option<Option<String>>,

    /// Updated event subscriptions.
    pub events: Option<Vec<Option<String>>>,

    /// Updated custom headers.
    pub headers: Option<serde_json::Value>,

    /// Updated status.
    pub status: Option<WebhookStatus>,

    /// Updated failure count.
    pub failure_count: Option<i32>,

    /// Updated max failures.
    pub max_failures: Option<i32>,

    /// Updated last triggered timestamp.
    pub last_triggered_at: Option<Option<Timestamp>>,

    /// Updated last success timestamp.
    pub last_success_at: Option<Option<Timestamp>>,

    /// Updated last failure timestamp.
    pub last_failure_at: Option<Option<Timestamp>>,

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

    /// Returns whether the webhook has reached its failure threshold.
    pub fn has_exceeded_failures(&self) -> bool {
        self.failure_count >= self.max_failures
    }

    /// Returns whether the webhook has a secret configured.
    pub fn has_secret(&self) -> bool {
        self.secret.is_some()
    }

    /// Returns whether the webhook has custom headers.
    pub fn has_custom_headers(&self) -> bool {
        !self.headers.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns the list of subscribed events as strings.
    pub fn subscribed_events(&self) -> Vec<&str> {
        self.events.iter().filter_map(|e| e.as_deref()).collect()
    }

    /// Returns whether the webhook subscribes to a specific event type.
    pub fn subscribes_to(&self, event: &str) -> bool {
        self.events.iter().any(|e| e.as_deref() == Some(event))
    }

    /// Returns whether the webhook has been successfully triggered at least once.
    pub fn has_been_triggered(&self) -> bool {
        self.last_success_at.is_some()
    }

    /// Returns whether the webhook is in a healthy state.
    pub fn is_healthy(&self) -> bool {
        self.is_active() && !self.has_exceeded_failures()
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
