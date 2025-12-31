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
    /// When the notification was created.
    pub created_at: Timestamp,
    /// When the notification expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
}

/// List of notifications.
pub type Notifications = Vec<Notification>;

impl Notification {
    /// Creates a Notification response from a database model.
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

    /// Creates a list of Notification responses from database models.
    pub fn from_models(models: Vec<AccountNotification>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }
}
