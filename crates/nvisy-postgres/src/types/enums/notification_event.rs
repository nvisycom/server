//! Notification event enumeration for user notifications.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of notification event sent to a user.
///
/// This enumeration corresponds to the `NOTIFICATION_EVENT` PostgreSQL enum and is used
/// for various user notifications including mentions, replies, and system announcements.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::NotificationEvent"]
pub enum NotificationEvent {
    // Comment events
    /// User was mentioned in a comment
    #[db_rename = "comment:mention"]
    #[serde(rename = "comment:mention")]
    #[default]
    CommentMention,

    /// Someone replied to user's comment
    #[db_rename = "comment:reply"]
    #[serde(rename = "comment:reply")]
    CommentReply,

    // Document events
    /// Document was uploaded
    #[db_rename = "document:uploaded"]
    #[serde(rename = "document:uploaded")]
    DocumentUploaded,

    /// Document was downloaded
    #[db_rename = "document:downloaded"]
    #[serde(rename = "document:downloaded")]
    DocumentDownloaded,

    /// Document verification completed
    #[db_rename = "document:verified"]
    #[serde(rename = "document:verified")]
    DocumentVerified,

    // Member events
    /// User was invited to a workspace
    #[db_rename = "member:invited"]
    #[serde(rename = "member:invited")]
    MemberInvited,

    /// A new member joined a workspace
    #[db_rename = "member:joined"]
    #[serde(rename = "member:joined")]
    MemberJoined,

    // Integration events
    /// Integration sync completed
    #[db_rename = "integration:synced"]
    #[serde(rename = "integration:synced")]
    IntegrationSynced,

    /// Integration sync failed or disconnected
    #[db_rename = "integration:desynced"]
    #[serde(rename = "integration:desynced")]
    IntegrationDesynced,

    // System events
    /// System-wide announcement
    #[db_rename = "system:announcement"]
    #[serde(rename = "system:announcement")]
    SystemAnnouncement,

    /// System report generated
    #[db_rename = "system:report"]
    #[serde(rename = "system:report")]
    SystemReport,
}

impl NotificationEvent {
    /// Returns whether this is a comment-related event.
    #[inline]
    pub fn is_comment_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::CommentMention | NotificationEvent::CommentReply
        )
    }

    /// Returns whether this is a document-related event.
    #[inline]
    pub fn is_document_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::DocumentUploaded
                | NotificationEvent::DocumentDownloaded
                | NotificationEvent::DocumentVerified
        )
    }

    /// Returns whether this is a member-related event.
    #[inline]
    pub fn is_member_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::MemberInvited | NotificationEvent::MemberJoined
        )
    }

    /// Returns whether this is an integration-related event.
    #[inline]
    pub fn is_integration_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::IntegrationSynced | NotificationEvent::IntegrationDesynced
        )
    }

    /// Returns whether this is a system-related event.
    #[inline]
    pub fn is_system_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::SystemAnnouncement | NotificationEvent::SystemReport
        )
    }

    /// Returns the event category as a string.
    pub fn category(&self) -> &'static str {
        match self {
            NotificationEvent::CommentMention | NotificationEvent::CommentReply => "comment",
            NotificationEvent::DocumentUploaded
            | NotificationEvent::DocumentDownloaded
            | NotificationEvent::DocumentVerified => "document",
            NotificationEvent::MemberInvited | NotificationEvent::MemberJoined => "member",
            NotificationEvent::IntegrationSynced | NotificationEvent::IntegrationDesynced => {
                "integration"
            }
            NotificationEvent::SystemAnnouncement | NotificationEvent::SystemReport => "system",
        }
    }
}
