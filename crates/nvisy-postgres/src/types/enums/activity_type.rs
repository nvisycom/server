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

    // Integration activities
    /// Integration was created
    #[db_rename = "integration:created"]
    #[serde(rename = "integration:created")]
    IntegrationCreated,

    /// Integration was updated
    #[db_rename = "integration:updated"]
    #[serde(rename = "integration:updated")]
    IntegrationUpdated,

    /// Integration was deleted
    #[db_rename = "integration:deleted"]
    #[serde(rename = "integration:deleted")]
    IntegrationDeleted,

    /// Integration completed synchronization
    #[db_rename = "integration:synced"]
    #[serde(rename = "integration:synced")]
    IntegrationSynced,

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

    // Document activities
    /// Document was created
    #[db_rename = "document:created"]
    #[serde(rename = "document:created")]
    DocumentCreated,

    /// Document was updated
    #[db_rename = "document:updated"]
    #[serde(rename = "document:updated")]
    DocumentUpdated,

    /// Document was deleted
    #[db_rename = "document:deleted"]
    #[serde(rename = "document:deleted")]
    DocumentDeleted,

    /// Document was verified
    #[db_rename = "document:verified"]
    #[serde(rename = "document:verified")]
    DocumentVerified,

    // Comment activities
    /// Comment was added
    #[db_rename = "comment:added"]
    #[serde(rename = "comment:added")]
    CommentAdded,

    /// Comment was updated
    #[db_rename = "comment:updated"]
    #[serde(rename = "comment:updated")]
    CommentUpdated,

    /// Comment was deleted
    #[db_rename = "comment:deleted"]
    #[serde(rename = "comment:deleted")]
    CommentDeleted,

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

            ActivityType::IntegrationCreated
            | ActivityType::IntegrationUpdated
            | ActivityType::IntegrationDeleted
            | ActivityType::IntegrationSynced => ActivityCategory::Integration,

            ActivityType::WebhookCreated
            | ActivityType::WebhookUpdated
            | ActivityType::WebhookDeleted
            | ActivityType::WebhookTriggered => ActivityCategory::Webhook,

            ActivityType::DocumentCreated
            | ActivityType::DocumentUpdated
            | ActivityType::DocumentDeleted
            | ActivityType::DocumentVerified => ActivityCategory::Document,

            ActivityType::CommentAdded
            | ActivityType::CommentUpdated
            | ActivityType::CommentDeleted => ActivityCategory::Comment,

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
                | ActivityType::IntegrationCreated
                | ActivityType::WebhookCreated
                | ActivityType::DocumentCreated
                | ActivityType::CommentAdded
        )
    }

    /// Returns whether this activity type represents a deletion event.
    #[inline]
    pub fn is_deletion(self) -> bool {
        matches!(
            self,
            ActivityType::WorkspaceDeleted
                | ActivityType::MemberDeleted
                | ActivityType::IntegrationDeleted
                | ActivityType::WebhookDeleted
                | ActivityType::DocumentDeleted
                | ActivityType::CommentDeleted
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
    /// Integration-related activities
    Integration,
    /// Webhook-related activities
    Webhook,
    /// Document-related activities
    Document,
    /// Comment-related activities
    Comment,
    /// Custom activities
    Custom,
}
