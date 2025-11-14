//! Account notification model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::account_notifications;
use crate::types::NotificationType;

/// Account notification model representing user notifications.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountNotification {
    /// Unique notification identifier
    pub id: Uuid,
    /// Account receiving the notification
    pub account_id: Uuid,
    /// Type of notification
    pub notify_type: NotificationType,
    /// Notification title
    pub title: String,
    /// Notification message
    pub message: String,
    /// Whether notification has been read
    pub is_read: bool,
    /// Timestamp when notification was read
    pub read_at: Option<OffsetDateTime>,
    /// ID of related entity (comment, document, etc.)
    pub related_id: Option<Uuid>,
    /// Type of related entity
    pub related_type: Option<String>,
    /// Additional notification data
    pub metadata: serde_json::Value,
    /// Notification creation timestamp
    pub created_at: OffsetDateTime,
    /// Optional expiration timestamp
    pub expires_at: Option<OffsetDateTime>,
}

/// Data for creating a new account notification.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccountNotification {
    /// Account ID
    pub account_id: Uuid,
    /// Notification type
    pub notify_type: NotificationType,
    /// Notification title
    pub title: String,
    /// Notification message
    pub message: String,
    /// Related entity ID
    pub related_id: Option<Uuid>,
    /// Related entity type
    pub related_type: Option<String>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Expiration timestamp
    pub expires_at: Option<OffsetDateTime>,
}

/// Data for updating an account notification.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = account_notifications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountNotification {
    /// Mark as read/unread
    pub is_read: Option<bool>,
    /// Read timestamp
    pub read_at: Option<OffsetDateTime>,
}

impl AccountNotification {
    /// Returns whether this notification has been read.
    pub fn is_read(&self) -> bool {
        self.is_read
    }

    /// Returns whether this notification is unread.
    pub fn is_unread(&self) -> bool {
        !self.is_read
    }

    /// Returns whether this notification has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < OffsetDateTime::now_utc()
        } else {
            false
        }
    }

    /// Returns whether this notification is active (not expired and not read).
    pub fn is_active(&self) -> bool {
        !self.is_read && !self.is_expired()
    }

    /// Returns whether this notification was created recently (within last 24 hours).
    pub fn is_recent(&self) -> bool {
        let now = OffsetDateTime::now_utc();
        let duration = now - self.created_at;
        duration.whole_days() < 1
    }

    /// Returns whether this notification is related to a specific entity.
    pub fn has_related_entity(&self) -> bool {
        self.related_id.is_some() && self.related_type.is_some()
    }

    /// Returns the related entity type and ID as a tuple if available.
    pub fn get_related_entity(&self) -> Option<(String, Uuid)> {
        if let (Some(entity_type), Some(entity_id)) = (&self.related_type, self.related_id) {
            Some((entity_type.clone(), entity_id))
        } else {
            None
        }
    }
}

impl NewAccountNotification {
    /// Creates a new comment mention notification.
    pub fn comment_mention(account_id: Uuid, commenter_name: &str, comment_id: Uuid) -> Self {
        Self {
            account_id,
            notify_type: NotificationType::CommentMention,
            title: "You were mentioned in a comment".to_string(),
            message: format!("{} mentioned you in a comment", commenter_name),
            related_id: Some(comment_id),
            related_type: Some("comment".to_string()),
            expires_at: Some(
                OffsetDateTime::now_utc()
                    + time::Duration::seconds(
                        NotificationType::CommentMention
                            .default_expiration_seconds()
                            .unwrap_or(2592000) as i64,
                    ),
            ),
            ..Default::default()
        }
    }

    /// Creates a new comment reply notification.
    pub fn comment_reply(account_id: Uuid, replier_name: &str, comment_id: Uuid) -> Self {
        Self {
            account_id,
            notify_type: NotificationType::CommentReply,
            title: "New reply to your comment".to_string(),
            message: format!("{} replied to your comment", replier_name),
            related_id: Some(comment_id),
            related_type: Some("comment".to_string()),
            expires_at: Some(
                OffsetDateTime::now_utc()
                    + time::Duration::seconds(
                        NotificationType::CommentReply
                            .default_expiration_seconds()
                            .unwrap_or(2592000) as i64,
                    ),
            ),
            ..Default::default()
        }
    }

    /// Creates a new project invite notification.
    pub fn project_invite(
        account_id: Uuid,
        inviter_name: &str,
        project_name: &str,
        project_id: Uuid,
    ) -> Self {
        Self {
            account_id,
            notify_type: NotificationType::ProjectInvite,
            title: "Project invitation".to_string(),
            message: format!("{} invited you to join \"{}\"", inviter_name, project_name),
            related_id: Some(project_id),
            related_type: Some("project".to_string()),
            expires_at: Some(
                OffsetDateTime::now_utc()
                    + time::Duration::seconds(
                        NotificationType::ProjectInvite
                            .default_expiration_seconds()
                            .unwrap_or(604800) as i64,
                    ),
            ),
            ..Default::default()
        }
    }

    /// Creates a new document upload notification.
    pub fn document_upload(account_id: Uuid, document_name: &str, document_id: Uuid) -> Self {
        Self {
            account_id,
            notify_type: NotificationType::DocumentUpload,
            title: "Document uploaded".to_string(),
            message: format!("\"{}\" has been uploaded successfully", document_name),
            related_id: Some(document_id),
            related_type: Some("document".to_string()),
            expires_at: Some(
                OffsetDateTime::now_utc()
                    + time::Duration::seconds(
                        NotificationType::DocumentUpload
                            .default_expiration_seconds()
                            .unwrap_or(7776000) as i64,
                    ),
            ),
            ..Default::default()
        }
    }

    /// Creates a new document download notification.
    pub fn document_download(
        account_id: Uuid,
        document_name: &str,
        document_id: Uuid,
        downloader_name: &str,
    ) -> Self {
        Self {
            account_id,
            notify_type: NotificationType::DocumentDownload,
            title: "Document downloaded".to_string(),
            message: format!("{} downloaded \"{}\"", downloader_name, document_name),
            related_id: Some(document_id),
            related_type: Some("document".to_string()),
            expires_at: Some(
                OffsetDateTime::now_utc()
                    + time::Duration::seconds(
                        NotificationType::DocumentDownload
                            .default_expiration_seconds()
                            .unwrap_or(7776000) as i64,
                    ),
            ),
            ..Default::default()
        }
    }

    /// Creates a new document verification notification.
    pub fn document_verify(
        account_id: Uuid,
        document_name: &str,
        document_id: Uuid,
        verification_status: &str,
    ) -> Self {
        Self {
            account_id,
            notify_type: NotificationType::DocumentVerify,
            title: "Document verification completed".to_string(),
            message: format!(
                "\"{}\" verification status: {}",
                document_name, verification_status
            ),
            related_id: Some(document_id),
            related_type: Some("document".to_string()),
            expires_at: Some(
                OffsetDateTime::now_utc()
                    + time::Duration::seconds(
                        NotificationType::DocumentVerify
                            .default_expiration_seconds()
                            .unwrap_or(7776000) as i64,
                    ),
            ),
            ..Default::default()
        }
    }

    /// Creates a new system announcement notification.
    pub fn system_announcement(account_id: Uuid, title: String, message: String) -> Self {
        Self {
            account_id,
            notify_type: NotificationType::SystemAnnouncement,
            title,
            message,
            related_id: None,
            related_type: None,
            expires_at: None, // System announcements don't expire
            ..Default::default()
        }
    }

    /// Sets custom metadata for the notification.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Sets a custom expiration time.
    pub fn with_expiration(mut self, expires_at: OffsetDateTime) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}
