//! Activity type enumeration for project audit logging.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of activity performed in a project for audit logging.
///
/// This enumeration corresponds to the `ACTIVITY_TYPE` PostgreSQL enum and is used
/// to categorize different types of activities that occur within projects for comprehensive
/// audit trail and activity tracking.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ActivityType"]
pub enum ActivityType {
    // Project activities
    /// Project was created
    #[db_rename = "project:created"]
    #[serde(rename = "project:created")]
    ProjectCreated,

    /// Project settings or metadata were updated
    #[db_rename = "project:updated"]
    #[serde(rename = "project:updated")]
    ProjectUpdated,

    /// Project was deleted
    #[db_rename = "project:deleted"]
    #[serde(rename = "project:deleted")]
    ProjectDeleted,

    /// Project was archived
    #[db_rename = "project:archived"]
    #[serde(rename = "project:archived")]
    ProjectArchived,

    /// Project was restored from archived state
    #[db_rename = "project:restored"]
    #[serde(rename = "project:restored")]
    ProjectRestored,

    /// Project settings were changed
    #[db_rename = "project:settings_changed"]
    #[serde(rename = "project:settings_changed")]
    ProjectSettingsChanged,

    /// Project was exported
    #[db_rename = "project:exported"]
    #[serde(rename = "project:exported")]
    ProjectExported,

    /// Project was imported
    #[db_rename = "project:imported"]
    #[serde(rename = "project:imported")]
    ProjectImported,

    // Member activities
    /// Member was added to the project
    #[db_rename = "member:added"]
    #[serde(rename = "member:added")]
    MemberAdded,

    /// Member was kicked from the project
    #[db_rename = "member:kicked"]
    #[serde(rename = "member:kicked")]
    MemberKicked,

    /// Member information or preferences were updated
    #[db_rename = "member:updated"]
    #[serde(rename = "member:updated")]
    MemberUpdated,

    /// Member was invited to the project
    #[db_rename = "member:invited"]
    #[serde(rename = "member:invited")]
    MemberInvited,

    /// Member accepted an invitation
    #[db_rename = "member:invite_accepted"]
    #[serde(rename = "member:invite_accepted")]
    MemberInviteAccepted,

    /// Member declined an invitation
    #[db_rename = "member:invite_declined"]
    #[serde(rename = "member:invite_declined")]
    MemberInviteDeclined,

    /// Invitation was canceled
    #[db_rename = "member:invite_canceled"]
    #[serde(rename = "member:invite_canceled")]
    MemberInviteCanceled,

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

    /// Integration was enabled
    #[db_rename = "integration:enabled"]
    #[serde(rename = "integration:enabled")]
    IntegrationEnabled,

    /// Integration was disabled
    #[db_rename = "integration:disabled"]
    #[serde(rename = "integration:disabled")]
    IntegrationDisabled,

    /// Integration completed synchronization
    #[db_rename = "integration:synced"]
    #[serde(rename = "integration:synced")]
    IntegrationSynced,

    /// Integration succeeded
    #[db_rename = "integration:succeeded"]
    #[serde(rename = "integration:succeeded")]
    IntegrationSucceeded,

    /// Integration encountered a failure
    #[db_rename = "integration:failed"]
    #[serde(rename = "integration:failed")]
    IntegrationFailed,

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

    /// Webhook was enabled
    #[db_rename = "webhook:enabled"]
    #[serde(rename = "webhook:enabled")]
    WebhookEnabled,

    /// Webhook was disabled
    #[db_rename = "webhook:disabled"]
    #[serde(rename = "webhook:disabled")]
    WebhookDisabled,

    /// Webhook was triggered
    #[db_rename = "webhook:triggered"]
    #[serde(rename = "webhook:triggered")]
    WebhookTriggered,

    /// Webhook delivery succeeded
    #[db_rename = "webhook:succeeded"]
    #[serde(rename = "webhook:succeeded")]
    WebhookSucceeded,

    /// Webhook delivery failed
    #[db_rename = "webhook:failed"]
    #[serde(rename = "webhook:failed")]
    WebhookFailed,

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

    /// Document was processed
    #[db_rename = "document:processed"]
    #[serde(rename = "document:processed")]
    DocumentProcessed,

    /// Document file was uploaded
    #[db_rename = "document:uploaded"]
    #[serde(rename = "document:uploaded")]
    DocumentUploaded,

    /// Document was downloaded
    #[db_rename = "document:downloaded"]
    #[serde(rename = "document:downloaded")]
    DocumentDownloaded,

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
            ActivityType::ProjectCreated
            | ActivityType::ProjectUpdated
            | ActivityType::ProjectDeleted
            | ActivityType::ProjectArchived
            | ActivityType::ProjectRestored
            | ActivityType::ProjectSettingsChanged
            | ActivityType::ProjectExported
            | ActivityType::ProjectImported => ActivityCategory::Project,

            ActivityType::MemberAdded
            | ActivityType::MemberKicked
            | ActivityType::MemberUpdated
            | ActivityType::MemberInvited
            | ActivityType::MemberInviteAccepted
            | ActivityType::MemberInviteDeclined
            | ActivityType::MemberInviteCanceled => ActivityCategory::Member,

            ActivityType::IntegrationCreated
            | ActivityType::IntegrationUpdated
            | ActivityType::IntegrationDeleted
            | ActivityType::IntegrationEnabled
            | ActivityType::IntegrationDisabled
            | ActivityType::IntegrationSynced
            | ActivityType::IntegrationSucceeded
            | ActivityType::IntegrationFailed => ActivityCategory::Integration,

            ActivityType::WebhookCreated
            | ActivityType::WebhookUpdated
            | ActivityType::WebhookDeleted
            | ActivityType::WebhookEnabled
            | ActivityType::WebhookDisabled
            | ActivityType::WebhookTriggered
            | ActivityType::WebhookSucceeded
            | ActivityType::WebhookFailed => ActivityCategory::Webhook,

            ActivityType::DocumentCreated
            | ActivityType::DocumentUpdated
            | ActivityType::DocumentDeleted
            | ActivityType::DocumentProcessed
            | ActivityType::DocumentUploaded
            | ActivityType::DocumentDownloaded
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
            ActivityType::ProjectCreated
                | ActivityType::MemberAdded
                | ActivityType::MemberInvited
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
            ActivityType::ProjectDeleted
                | ActivityType::MemberKicked
                | ActivityType::IntegrationDeleted
                | ActivityType::WebhookDeleted
                | ActivityType::DocumentDeleted
                | ActivityType::CommentDeleted
        )
    }

    /// Returns whether this activity type represents a security-sensitive event.
    #[inline]
    pub fn is_security_sensitive(self) -> bool {
        matches!(self.category(), ActivityCategory::Member)
    }
}

/// Categories for grouping activity types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ActivityCategory {
    /// Project-related activities
    Project,
    /// Member-related activities
    Member,
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
