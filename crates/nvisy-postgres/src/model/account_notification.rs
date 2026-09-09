//! Account notification model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_notifications;
use crate::types::{Json, NotificationEvent, NotificationPayload};

/// Account notification model representing a notification sent to a user.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountNotification {
    /// Unique notification identifier.
    pub id: Uuid,
    /// Account receiving the notification.
    pub account_id: Uuid,
    /// Notification type; the client-side localization key.
    pub notify_type: NotificationEvent,
    /// When the notification was read; `None` means unread.
    pub read_at: Option<Timestamp>,
    /// The self-describing tagged payload (its `type` tag + params).
    pub params: Json<NotificationPayload>,
    /// Notification creation timestamp.
    pub created_at: Timestamp,
    /// Optional expiration timestamp.
    pub expires_at: Option<Timestamp>,
}

/// Data for creating a new account notification.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccountNotification {
    /// Account ID.
    pub account_id: Uuid,
    /// Notification type; the client-side localization key.
    pub notify_type: NotificationEvent,
    /// The self-describing tagged payload (its `type` tag + params).
    pub params: Json<NotificationPayload>,
    /// Expiration timestamp.
    pub expires_at: Option<Timestamp>,
}

/// Data for updating an account notification.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountNotification {
    /// Read timestamp: `Some(Some(ts))` marks read, `Some(None)` marks unread.
    pub read_at: Option<Option<Timestamp>>,
}
