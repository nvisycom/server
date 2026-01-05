//! Account notification response types.

use jiff::Timestamp;
use nvisy_postgres::model::AccountNotification;
use nvisy_postgres::types::NotificationEvent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for an account notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Unique notification identifier.
    pub id: Uuid,
    /// Notification type.
    pub notify_type: NotificationEvent,
    /// Notification title.
    pub title: String,
    /// Notification message.
    pub message: String,
    /// Whether the notification has been read.
    pub is_read: bool,
    /// When the notification was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<Timestamp>,
    /// Related entity ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_id: Option<Uuid>,
    /// Related entity type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_type: Option<String>,
    /// When the notification was created.
    pub created_at: Timestamp,
    /// When the notification expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
}

/// Paginated list of notifications.
pub type NotificationsPage = Page<Notification>;

/// Response type for unread notifications status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnreadStatus {
    /// Number of unread notifications.
    pub unread_count: i64,
}

impl Notification {
    pub fn from_model(notification: AccountNotification) -> Self {
        Self {
            id: notification.id,
            notify_type: notification.notify_type,
            title: notification.title,
            message: notification.message,
            is_read: notification.is_read,
            read_at: notification.read_at.map(Into::into),
            related_id: notification.related_id,
            related_type: notification.related_type,
            created_at: notification.created_at.into(),
            expires_at: notification.expires_at.map(Into::into),
        }
    }
}
