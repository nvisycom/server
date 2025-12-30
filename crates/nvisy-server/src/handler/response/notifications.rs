//! Account notification response types.

use jiff::Timestamp;
use nvisy_postgres::model::AccountNotification;
use nvisy_postgres::types::NotificationType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response type for an account notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Unique notification identifier.
    pub id: Uuid,
    /// Notification type.
    pub notify_type: NotificationType,
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
    /// Additional metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// When the notification was created.
    pub created_at: Timestamp,
    /// When the notification expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
}

/// List of notifications.
pub type Notifications = Vec<Notification>;

impl From<AccountNotification> for Notification {
    fn from(notification: AccountNotification) -> Self {
        let metadata = if notification
            .metadata
            .as_object()
            .is_none_or(|obj| obj.is_empty())
        {
            None
        } else {
            Some(notification.metadata)
        };

        Self {
            id: notification.id,
            notify_type: notification.notify_type,
            title: notification.title,
            message: notification.message,
            is_read: notification.is_read,
            read_at: notification.read_at.map(Into::into),
            related_id: notification.related_id,
            related_type: notification.related_type,
            metadata,
            created_at: notification.created_at.into(),
            expires_at: notification.expires_at.map(Into::into),
        }
    }
}
