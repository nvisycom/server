//! Notification event enumeration for user notifications.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of notification event sent to a user.
///
/// This enumeration corresponds to the `NOTIFICATION_EVENT` PostgreSQL enum and
/// is used for member, connection-sync, pipeline-run, and system notifications.
/// The values mirror the [`WebhookEvent`](super::WebhookEvent) naming for the
/// events the two channels share.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::NotificationEvent"]
pub enum NotificationEvent {
    // Member events
    /// User was invited to a workspace
    #[db_rename = "member:invited"]
    #[serde(rename = "member:invited")]
    MemberInvited,

    /// A new member joined a workspace
    #[db_rename = "member:joined"]
    #[serde(rename = "member:joined")]
    MemberJoined,

    // Connection sync events
    /// A connection sync completed
    #[db_rename = "connection:sync.completed"]
    #[serde(rename = "connection:sync.completed")]
    ConnectionSyncCompleted,

    /// A connection sync failed
    #[db_rename = "connection:sync.failed"]
    #[serde(rename = "connection:sync.failed")]
    ConnectionSyncFailed,

    // Pipeline run events
    /// A pipeline run finished detection and is awaiting review
    #[db_rename = "pipeline:run.analyzed"]
    #[serde(rename = "pipeline:run.analyzed")]
    PipelineRunAnalyzed,

    /// A pipeline run completed (redaction produced)
    #[db_rename = "pipeline:run.completed"]
    #[serde(rename = "pipeline:run.completed")]
    PipelineRunCompleted,

    /// A pipeline run failed
    #[db_rename = "pipeline:run.failed"]
    #[serde(rename = "pipeline:run.failed")]
    PipelineRunFailed,

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
            NotificationEvent::ConnectionSyncCompleted | NotificationEvent::ConnectionSyncFailed
        )
    }

    /// Returns whether this is a pipeline-related event.
    #[inline]
    pub fn is_pipeline_event(self) -> bool {
        matches!(
            self,
            NotificationEvent::PipelineRunAnalyzed
                | NotificationEvent::PipelineRunCompleted
                | NotificationEvent::PipelineRunFailed
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
            NotificationEvent::MemberInvited | NotificationEvent::MemberJoined => "member",
            NotificationEvent::ConnectionSyncCompleted
            | NotificationEvent::ConnectionSyncFailed => "connection",
            NotificationEvent::PipelineRunAnalyzed
            | NotificationEvent::PipelineRunCompleted
            | NotificationEvent::PipelineRunFailed => "pipeline",
            NotificationEvent::SystemAnnouncement | NotificationEvent::SystemReport => "system",
        }
    }
}
