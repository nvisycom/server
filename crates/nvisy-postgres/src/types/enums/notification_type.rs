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

    /// User was invited to a workspace
    #[db_rename = "workspace_invite"]
    #[serde(rename = "workspace_invite")]
    WorkspaceInvite,

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
                | NotificationType::WorkspaceInvite
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
}
