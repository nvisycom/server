//! Account notification model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_notifications;
use crate::types::constants::notification;
use crate::types::{HasCreatedAt, HasExpiresAt, NotificationType};

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
    pub read_at: Option<Timestamp>,
    /// ID of related entity (comment, document, etc.).
    pub related_id: Option<Uuid>,
    /// Type of related entity.
    pub related_type: Option<String>,
    /// Additional notification data.
    pub metadata: serde_json::Value,
    /// Notification creation timestamp.
    pub created_at: Timestamp,
    /// Optional expiration timestamp.
    pub expires_at: Option<Timestamp>,
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
    pub expires_at: Option<Timestamp>,
}

/// Data for updating an account notification.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountNotification {
    /// Mark as read/unread.
    pub is_read: Option<bool>,
    /// Read timestamp.
    pub read_at: Option<Timestamp>,
}

impl AccountNotification {
    /// Returns whether the notification is currently unread.
    pub fn is_unread(&self) -> bool {
        !self.is_read
    }

    /// Returns whether the notification has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            jiff::Timestamp::now() > jiff::Timestamp::from(expires_at)
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

    /// Returns the time remaining until expiration.
    pub fn time_until_expiry(&self) -> Option<jiff::Span> {
        if let Some(expires_at) = self.expires_at {
            let now = jiff::Timestamp::now();
            let expires_at = jiff::Timestamp::from(expires_at);
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
    pub fn age(&self) -> jiff::Span {
        jiff::Timestamp::now() - jiff::Timestamp::from(self.created_at)
    }

    /// Returns whether the notification is expiring soon (within 24 hours).
    pub fn is_expiring_soon(&self) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining.total(jiff::Unit::Second).ok()
                <= jiff::Span::new().days(1).total(jiff::Unit::Second).ok()
        } else {
            false
        }
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
                | NotificationType::WorkspaceInvite
        )
    }

    /// Returns whether the notification can be dismissed.
    pub fn can_be_dismissed(&self) -> bool {
        !self.is_system_notification() || self.is_read
    }

    /// Returns whether the notification should be shown to the user.
    pub fn should_display(&self) -> bool {
        !self.is_expired() && (!self.is_read || self.is_system_notification())
    }

    /// Returns whether the notification has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the notification requires action from the user.
    pub fn requires_action(&self) -> bool {
        matches!(
            self.notify_type,
            NotificationType::WorkspaceInvite | NotificationType::SystemAnnouncement
        )
    }

    /// Returns the notification priority level (0 = low, 2 = high).
    pub fn priority_level(&self) -> u8 {
        if self.is_system_notification() {
            2
        } else if self.requires_action() {
            1
        } else {
            0
        }
    }
}

impl HasCreatedAt for AccountNotification {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasExpiresAt for AccountNotification {
    fn expires_at(&self) -> jiff::Timestamp {
        self.expires_at.map(Into::into).unwrap_or(
            jiff::Timestamp::now()
                + jiff::Span::new().days(notification::DEFAULT_RETENTION_DAYS as i64),
        )
    }
}
