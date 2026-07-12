//! Notification event enumeration for user notifications.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of notification event sent to a user.
///
/// This enumeration corresponds to the `NOTIFICATION_EVENT` PostgreSQL enum and is used
/// for various user notifications including file, member, connection, and system events.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::NotificationEvent"]
pub enum NotificationEvent {
    // File events
    /// File was uploaded
    #[db_rename = "file:uploaded"]
    #[serde(rename = "file:uploaded")]
    FileUploaded,

    /// File was downloaded
    #[db_rename = "file:downloaded"]
    #[serde(rename = "file:downloaded")]
    FileDownloaded,

    /// File verification completed
    #[db_rename = "file:verified"]
    #[serde(rename = "file:verified")]
    FileVerified,

    // Member events
    /// User was invited to a workspace
    #[db_rename = "member:invited"]
    #[serde(rename = "member:invited")]
    MemberInvited,

    /// A new member joined a workspace
    #[db_rename = "member:joined"]
    #[serde(rename = "member:joined")]
    MemberJoined,

    // Connection events
    /// Connection sync completed
    #[db_rename = "connection:synced"]
    #[serde(rename = "connection:synced")]
    ConnectionSynced,

    /// Connection sync failed or disconnected
    #[db_rename = "connection:desynced"]
    #[serde(rename = "connection:desynced")]
    ConnectionDesynced,

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
    /// Returns whether this is a file-related event.
    #[inline]
    pub fn is_file_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::FileUploaded
                | NotificationEvent::FileDownloaded
                | NotificationEvent::FileVerified
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

    /// Returns whether this is a connection-related event.
    #[inline]
    pub fn is_connection_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::ConnectionSynced | NotificationEvent::ConnectionDesynced
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
            NotificationEvent::FileUploaded
            | NotificationEvent::FileDownloaded
            | NotificationEvent::FileVerified => "file",
            NotificationEvent::MemberInvited | NotificationEvent::MemberJoined => "member",
            NotificationEvent::ConnectionSynced | NotificationEvent::ConnectionDesynced => {
                "connection"
            }
            NotificationEvent::SystemAnnouncement | NotificationEvent::SystemReport => "system",
        }
    }
}
