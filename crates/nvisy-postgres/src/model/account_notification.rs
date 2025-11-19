//! Account notification model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::account_notifications;
use crate::types::constants::notification;
use crate::types::{HasCreatedAt, HasExpiresAt, NotificationType, is_within_duration};

/// Account notification model representing a notification sent to a user.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountNotification {
    /// Unique notification identifier.
    pub id: Uuid,
    /// Account receiving the notification.
    pub account_id: Uuid,
    /// Type of notification.
    pub notify_type: NotificationType,
    /// Notification title.
    pub title: String,
    /// Notification message.
    pub message: String,
    /// Whether notification has been read.
    pub is_read: bool,
    /// Timestamp when notification was read.
    pub read_at: Option<OffsetDateTime>,
    /// ID of related entity (comment, document, etc.).
    pub related_id: Option<Uuid>,
    /// Type of related entity.
    pub related_type: Option<String>,
    /// Additional notification data.
    pub metadata: serde_json::Value,
    /// Notification creation timestamp.
    pub created_at: OffsetDateTime,
    /// Optional expiration timestamp.
    pub expires_at: Option<OffsetDateTime>,
}

/// Data for creating a new account notification.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccountNotification {
    /// Account ID.
    pub account_id: Uuid,
    /// Notification type.
    pub notify_type: NotificationType,
    /// Notification title.
    pub title: String,
    /// Notification message.
    pub message: String,
    /// Related entity ID.
    pub related_id: Option<Uuid>,
    /// Related entity type.
    pub related_type: Option<String>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Expiration timestamp.
    pub expires_at: Option<OffsetDateTime>,
}

/// Data for updating an account notification.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountNotification {
    /// Mark as read/unread.
    pub is_read: Option<bool>,
    /// Read timestamp.
    pub read_at: Option<OffsetDateTime>,
}

impl AccountNotification {
    /// Returns whether the notification is currently unread.
    pub fn is_unread(&self) -> bool {
        !self.is_read
    }

    /// Returns whether the notification has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            OffsetDateTime::now_utc() > expires_at
        } else {
            false
        }
    }

    /// Returns whether the notification is active (unread and not expired).
    pub fn is_active(&self) -> bool {
        !self.is_read && !self.is_expired()
    }

    /// Returns whether the notification has a related entity.
    pub fn has_related_entity(&self) -> bool {
        self.related_id.is_some()
    }

    /// Returns whether the notification was read recently (within last hour).
    pub fn is_recently_read(&self) -> bool {
        if let Some(read_at) = self.read_at {
            is_within_duration(read_at, time::Duration::hours(1))
        } else {
            false
        }
    }

    /// Returns the time remaining until expiration.
    pub fn time_until_expiry(&self) -> Option<time::Duration> {
        if let Some(expires_at) = self.expires_at {
            let now = OffsetDateTime::now_utc();
            if expires_at > now {
                Some(expires_at - now)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Returns the duration since the notification was created.
    pub fn age(&self) -> time::Duration {
        OffsetDateTime::now_utc() - self.created_at
    }

    /// Returns whether the notification is expiring soon (within 24 hours).
    pub fn is_expiring_soon(&self) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining <= time::Duration::days(1)
        } else {
            false
        }
    }

    /// Returns whether this is a high-priority notification.
    pub fn is_high_priority(&self) -> bool {
        matches!(self.notify_type, NotificationType::SystemAnnouncement)
    }

    /// Returns whether this is a system notification.
    pub fn is_system_notification(&self) -> bool {
        matches!(self.notify_type, NotificationType::SystemAnnouncement)
    }

    /// Returns whether this is a user activity notification.
    pub fn is_user_activity(&self) -> bool {
        matches!(
            self.notify_type,
            NotificationType::CommentMention
                | NotificationType::CommentReply
                | NotificationType::ProjectInvite
        )
    }

    /// Returns whether the notification can be dismissed.
    pub fn can_be_dismissed(&self) -> bool {
        !self.is_high_priority() || self.is_read
    }

    /// Returns whether the notification should be shown to the user.
    pub fn should_display(&self) -> bool {
        !self.is_expired() && (!self.is_read || self.is_high_priority())
    }

    /// Returns whether the notification has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().map_or(true, |obj| obj.is_empty())
    }

    /// Returns whether the notification requires action from the user.
    pub fn requires_action(&self) -> bool {
        matches!(
            self.notify_type,
            NotificationType::ProjectInvite | NotificationType::SystemAnnouncement
        )
    }

    /// Returns the notification priority level (0 = low, 2 = high).
    pub fn priority_level(&self) -> u8 {
        if self.is_high_priority() {
            2
        } else if self.requires_action() {
            1
        } else {
            0
        }
    }
}

impl HasCreatedAt for AccountNotification {
    fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }
}

impl HasExpiresAt for AccountNotification {
    fn expires_at(&self) -> OffsetDateTime {
        self.expires_at.unwrap_or(
            OffsetDateTime::now_utc()
                + time::Duration::days(notification::DEFAULT_RETENTION_DAYS as i64),
        )
    }
}
