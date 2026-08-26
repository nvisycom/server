//! Notification event enumeration for user notifications.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of notification event sent to a user.
///
/// This enumeration corresponds to the `NOTIFICATION_EVENT` PostgreSQL enum and
/// is used for member, connection-sync, detection, redaction, and system
/// notifications.
/// The values mirror the [`WebhookEvent`](super::WebhookEvent) naming for the
/// events the two channels share.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::NotificationEvent"]
pub enum NotificationEvent {
    // Member events
    /// User was invited to a workspace
    #[db_rename = "member.invited"]
    #[serde(rename = "member.invited")]
    MemberInvited,

    /// A new member joined a workspace
    #[db_rename = "member.joined"]
    #[serde(rename = "member.joined")]
    MemberJoined,

    // Connection sync events
    /// A connection sync completed
    #[db_rename = "connection.sync.completed"]
    #[serde(rename = "connection.sync.completed")]
    ConnectionSyncCompleted,

    /// A connection sync failed
    #[db_rename = "connection.sync.failed"]
    #[serde(rename = "connection.sync.failed")]
    ConnectionSyncFailed,

    // Detection / redaction events
    /// A detection finished analysis and is ready to redact
    #[db_rename = "pipeline.detection.completed"]
    #[serde(rename = "pipeline.detection.completed")]
    DetectionCompleted,

    /// A redaction was created (redacted output produced)
    #[db_rename = "pipeline.redaction.created"]
    #[serde(rename = "pipeline.redaction.created")]
    RedactionCreated,

    /// A detection failed
    #[db_rename = "pipeline.detection.failed"]
    #[serde(rename = "pipeline.detection.failed")]
    DetectionFailed,
}
