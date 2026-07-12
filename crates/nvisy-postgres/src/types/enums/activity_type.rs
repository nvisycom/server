//! Activity type enumeration for workspace audit logging.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of activity performed in a workspace for audit logging.
///
/// This enumeration corresponds to the `ACTIVITY_TYPE` PostgreSQL enum and is used
/// to categorize different types of activities that occur within workspaces for comprehensive
/// audit trail and activity tracking.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ActivityType"]
pub enum ActivityType {
    // Workspace activities
    /// Workspace was created
    #[db_rename = "workspace:created"]
    #[serde(rename = "workspace:created")]
    WorkspaceCreated,

    /// Workspace settings or metadata were updated
    #[db_rename = "workspace:updated"]
    #[serde(rename = "workspace:updated")]
    WorkspaceUpdated,

    /// Workspace was deleted
    #[db_rename = "workspace:deleted"]
    #[serde(rename = "workspace:deleted")]
    WorkspaceDeleted,

    /// Workspace was exported
    #[db_rename = "workspace:exported"]
    #[serde(rename = "workspace:exported")]
    WorkspaceExported,

    /// Workspace was imported
    #[db_rename = "workspace:imported"]
    #[serde(rename = "workspace:imported")]
    WorkspaceImported,

    // Member activities
    /// Member was removed from the workspace
    #[db_rename = "member:deleted"]
    #[serde(rename = "member:deleted")]
    MemberDeleted,

    /// Member information or preferences were updated
    #[db_rename = "member:updated"]
    #[serde(rename = "member:updated")]
    MemberUpdated,

    // Invite activities
    /// Invite was created
    #[db_rename = "invite:created"]
    #[serde(rename = "invite:created")]
    InviteCreated,

    /// Invite was accepted
    #[db_rename = "invite:accepted"]
    #[serde(rename = "invite:accepted")]
    InviteAccepted,

    /// Invite was declined
    #[db_rename = "invite:declined"]
    #[serde(rename = "invite:declined")]
    InviteDeclined,

    /// Invite was canceled
    #[db_rename = "invite:canceled"]
    #[serde(rename = "invite:canceled")]
    InviteCanceled,

    // Connection activities
    /// Connection was created
    #[db_rename = "connection:created"]
    #[serde(rename = "connection:created")]
    ConnectionCreated,

    /// Connection was updated
    #[db_rename = "connection:updated"]
    #[serde(rename = "connection:updated")]
    ConnectionUpdated,

    /// Connection was deleted
    #[db_rename = "connection:deleted"]
    #[serde(rename = "connection:deleted")]
    ConnectionDeleted,

    /// Connection completed synchronization
    #[db_rename = "connection:synced"]
    #[serde(rename = "connection:synced")]
    ConnectionSynced,

    // Webhook activities
    /// Webhook was created
    #[db_rename = "webhook:created"]
    #[serde(rename = "webhook:created")]
    WebhookCreated,

    /// Webhook was updated
    #[db_rename = "webhook:updated"]
    #[serde(rename = "webhook:updated")]
    WebhookUpdated,

    /// Webhook was deleted
    #[db_rename = "webhook:deleted"]
    #[serde(rename = "webhook:deleted")]
    WebhookDeleted,

    /// Webhook was triggered
    #[db_rename = "webhook:triggered"]
    #[serde(rename = "webhook:triggered")]
    WebhookTriggered,

    // File activities
    /// File was created
    #[db_rename = "file:created"]
    #[serde(rename = "file:created")]
    FileCreated,

    /// File was updated
    #[db_rename = "file:updated"]
    #[serde(rename = "file:updated")]
    FileUpdated,

    /// File was deleted
    #[db_rename = "file:deleted"]
    #[serde(rename = "file:deleted")]
    FileDeleted,

    /// File was verified
    #[db_rename = "file:verified"]
    #[serde(rename = "file:verified")]
    FileVerified,

    // Custom activities
    /// Custom activity type for extensibility
    #[db_rename = "custom"]
    #[serde(rename = "custom")]
    #[default]
    Custom,
}

impl ActivityType {
    /// Returns the category of this activity type.
    #[inline]
    pub fn category(self) -> ActivityCategory {
        match self {
            ActivityType::WorkspaceCreated
            | ActivityType::WorkspaceUpdated
            | ActivityType::WorkspaceDeleted
            | ActivityType::WorkspaceExported
            | ActivityType::WorkspaceImported => ActivityCategory::Workspace,

            ActivityType::MemberDeleted | ActivityType::MemberUpdated => ActivityCategory::Member,

            ActivityType::InviteCreated
            | ActivityType::InviteAccepted
            | ActivityType::InviteDeclined
            | ActivityType::InviteCanceled => ActivityCategory::Invite,

            ActivityType::ConnectionCreated
            | ActivityType::ConnectionUpdated
            | ActivityType::ConnectionDeleted
            | ActivityType::ConnectionSynced => ActivityCategory::Connection,

            ActivityType::WebhookCreated
            | ActivityType::WebhookUpdated
            | ActivityType::WebhookDeleted
            | ActivityType::WebhookTriggered => ActivityCategory::Webhook,

            ActivityType::FileCreated
            | ActivityType::FileUpdated
            | ActivityType::FileDeleted
            | ActivityType::FileVerified => ActivityCategory::File,

            ActivityType::Custom => ActivityCategory::Custom,
        }
    }

    /// Returns whether this activity type represents a creation event.
    #[inline]
    pub fn is_creation(self) -> bool {
        matches!(
            self,
            ActivityType::WorkspaceCreated
                | ActivityType::InviteCreated
                | ActivityType::ConnectionCreated
                | ActivityType::WebhookCreated
                | ActivityType::FileCreated
        )
    }

    /// Returns whether this activity type represents a deletion event.
    #[inline]
    pub fn is_deletion(self) -> bool {
        matches!(
            self,
            ActivityType::WorkspaceDeleted
                | ActivityType::MemberDeleted
                | ActivityType::ConnectionDeleted
                | ActivityType::WebhookDeleted
                | ActivityType::FileDeleted
        )
    }

    /// Returns whether this activity type represents a security-sensitive event.
    #[inline]
    pub fn is_security_sensitive(self) -> bool {
        matches!(
            self.category(),
            ActivityCategory::Member | ActivityCategory::Invite
        )
    }
}

/// Categories for grouping activity types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ActivityCategory {
    /// Workspace-related activities
    Workspace,
    /// Member-related activities
    Member,
    /// Invite-related activities
    Invite,
    /// Connection-related activities
    Connection,
    /// Webhook-related activities
    Webhook,
    /// File-related activities
    File,
    /// Custom activities
    Custom,
}
