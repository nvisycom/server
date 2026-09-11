//! Account notification response types.

use jiff::Timestamp;
use nvisy_postgres::model::AccountNotification;
use nvisy_postgres::types::NotificationPayload;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Response type for an account notification.
///
/// The typed payload is nested under `payload`, so a notification is
/// `{ id, payload: { notifyType, <params...> }, readAt, ... }`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Unique notification identifier.
    pub id: Uuid,
    /// The notification type and its typed params, absent when the stored params
    /// do not decode into their `notifyType`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<NotificationPayload>,
    /// When the notification was read; absent means unread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<Timestamp>,
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

/// Response type for a mark-all-read action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkedReadStatus {
    /// Number of notifications the request marked as read.
    pub marked_read: i64,
}

impl Notification {
    /// Builds the response from a stored notification, reconstructing the typed
    /// payload from `notify_type` and the stored params.
    ///
    /// Total by design: a row whose stored params do not decode into their
    /// `notify_type` still appears, with `payload` absent rather than the row
    /// dropped. Dropping it would let the list silently disagree with the unread
    /// count.
    pub fn from_model(notification: AccountNotification) -> Self {
        Self {
            id: notification.id,
            payload: notification.params.optional(),
            read_at: notification.read_at.map(Into::into),
            created_at: notification.created_at.into(),
            expires_at: notification.expires_at.map(Into::into),
        }
    }
}
