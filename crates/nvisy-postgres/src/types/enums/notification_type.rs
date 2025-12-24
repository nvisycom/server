//! Notification type enumeration for user notifications.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of notification sent to a user.
///
/// This enumeration corresponds to the `NOTIFICATION_TYPE` PostgreSQL enum and is used
/// for various user notifications including mentions, replies, and system announcements.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::NotificationType"]
pub enum NotificationType {
    /// User was mentioned in a comment
    #[db_rename = "comment_mention"]
    #[serde(rename = "comment_mention")]
    #[default]
    CommentMention,

    /// Someone replied to user's comment
    #[db_rename = "comment_reply"]
    #[serde(rename = "comment_reply")]
    CommentReply,

    /// Document was uploaded
    #[db_rename = "document_upload"]
    #[serde(rename = "document_upload")]
    DocumentUpload,

    /// Document was downloaded
    #[db_rename = "document_download"]
    #[serde(rename = "document_download")]
    DocumentDownload,

    /// Document verification completed
    #[db_rename = "document_verify"]
    #[serde(rename = "document_verify")]
    DocumentVerify,

    /// User was invited to a project
    #[db_rename = "project_invite"]
    #[serde(rename = "project_invite")]
    ProjectInvite,

    /// System-wide announcement
    #[db_rename = "system_announcement"]
    #[serde(rename = "system_announcement")]
    SystemAnnouncement,
}

impl NotificationType {
    /// Returns whether this notification type is user-generated.
    #[inline]
    pub fn is_user_generated(self) -> bool {
        matches!(
            self,
            NotificationType::CommentMention
                | NotificationType::CommentReply
                | NotificationType::ProjectInvite
        )
    }

    /// Returns whether this notification type is system-generated.
    #[inline]
    pub fn is_system_generated(self) -> bool {
        matches!(
            self,
            NotificationType::SystemAnnouncement
                | NotificationType::DocumentUpload
                | NotificationType::DocumentDownload
                | NotificationType::DocumentVerify
        )
    }

    /// Returns whether this notification type is related to comments.
    #[inline]
    pub fn is_comment_related(self) -> bool {
        matches!(
            self,
            NotificationType::CommentMention | NotificationType::CommentReply
        )
    }

    /// Returns whether this notification type is related to projects.
    #[inline]
    pub fn is_project_related(self) -> bool {
        matches!(self, NotificationType::ProjectInvite)
    }

    /// Returns whether this notification type is related to documents.
    #[inline]
    pub fn is_document_related(self) -> bool {
        matches!(
            self,
            NotificationType::DocumentUpload
                | NotificationType::DocumentDownload
                | NotificationType::DocumentVerify
        )
    }

    /// Returns the default expiration time in seconds for this notification type.
    /// Returns None for notifications that don't expire.
    #[inline]
    pub fn default_expiration_seconds(self) -> Option<u32> {
        match self {
            // Comment notifications expire after 30 days
            NotificationType::CommentMention | NotificationType::CommentReply => {
                Some(30 * 24 * 60 * 60)
            }
            // Project invites expire after 7 days
            NotificationType::ProjectInvite => Some(7 * 24 * 60 * 60),
            // Document notifications expire after 90 days
            NotificationType::DocumentUpload
            | NotificationType::DocumentDownload
            | NotificationType::DocumentVerify => Some(90 * 24 * 60 * 60),
            // System announcements don't expire
            NotificationType::SystemAnnouncement => None,
        }
    }

    /// Returns the priority level of this notification type (1-5, 5 being highest).
    #[inline]
    pub fn priority(self) -> u8 {
        match self {
            NotificationType::SystemAnnouncement => 5,
            NotificationType::ProjectInvite => 4,
            NotificationType::CommentMention => 3,
            NotificationType::DocumentVerify => 2,
            NotificationType::DocumentUpload => 2,
            NotificationType::DocumentDownload => 1,
            NotificationType::CommentReply => 2,
        }
    }
}
