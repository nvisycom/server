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
#[must_use]
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
#[must_use]
pub struct NewAccountNotification {
    /// Account ID.
    pub account_id: Uuid,
    /// Notification type; the client-side localization key.
    pub notify_type: NotificationEvent,
    /// The self-describing tagged payload (its `type` tag + params).
    pub params: Json<NotificationPayload>,
    /// Expiration timestamp.
    pub expires_at: Option<Timestamp>,
    /// Creation timestamp override, for tests only.
    #[cfg(any(feature = "test_util", test))]
    pub created_at: Option<Timestamp>,
}

impl NewAccountNotification {
    /// A minimal `member.joined` notification for `account_id`, unread and
    /// non-expiring, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(account_id: Uuid) -> Self {
        use crate::types::{Handle, MemberJoinedParams, NotificationPayload};

        let payload = NotificationPayload::MemberJoined(MemberJoinedParams {
            workspace_slug: Handle::test(),
            member_username: Handle::test(),
        });
        let (notify_type, params) = payload.into_stored();
        Self {
            account_id,
            notify_type,
            params,
            expires_at: None,
            created_at: None,
        }
    }
}

/// Data for updating an account notification.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateAccountNotification {
    /// Read timestamp: `Some(Some(ts))` marks read, `Some(None)` marks unread.
    pub read_at: Option<Option<Timestamp>>,
}
