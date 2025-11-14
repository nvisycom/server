//! Activity type enumeration for project audit logging.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Defines the type of activity performed in a project for audit logging.
///
/// This enumeration corresponds to the `ACTIVITY_TYPE` PostgreSQL enum and is used
/// to categorize different types of activities that occur within projects for comprehensive
/// audit trail and activity tracking.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[ExistingTypePath = "crate::schema::sql_types::ActivityType"]
pub enum ActivityType {
    // Project activities
    /// Project was created
    #[db_rename = "project_created"]
    #[serde(rename = "project_created")]
    ProjectCreated,

    /// Project settings or metadata were updated
    #[db_rename = "project_updated"]
    #[serde(rename = "project_updated")]
    ProjectUpdated,

    /// Project was deleted
    #[db_rename = "project_deleted"]
    #[serde(rename = "project_deleted")]
    ProjectDeleted,

    /// Project was archived
    #[db_rename = "project_archived"]
    #[serde(rename = "project_archived")]
    ProjectArchived,

    /// Project was restored from archived state
    #[db_rename = "project_restored"]
    #[serde(rename = "project_restored")]
    ProjectRestored,

    /// Project settings were changed
    #[db_rename = "project_settings_changed"]
    #[serde(rename = "project_settings_changed")]
    ProjectSettingsChanged,

    /// Project was exported
    #[db_rename = "project_exported"]
    #[serde(rename = "project_exported")]
    ProjectExported,

    /// Project was imported
    #[db_rename = "project_imported"]
    #[serde(rename = "project_imported")]
    ProjectImported,

    // Member activities
    /// Member was added to the project
    #[db_rename = "member_added"]
    #[serde(rename = "member_added")]
    MemberAdded,

    /// Member was kicked from the project
    #[db_rename = "member_kicked"]
    #[serde(rename = "member_kicked")]
    MemberKicked,

    /// Member information or preferences were updated
    #[db_rename = "member_updated"]
    #[serde(rename = "member_updated")]
    MemberUpdated,

    /// Member was invited to the project
    #[db_rename = "member_invited"]
    #[serde(rename = "member_invited")]
    MemberInvited,

    /// Member accepted an invitation
    #[db_rename = "member_invite_accepted"]
    #[serde(rename = "member_invite_accepted")]
    MemberInviteAccepted,

    /// Member declined an invitation
    #[db_rename = "member_invite_declined"]
    #[serde(rename = "member_invite_declined")]
    MemberInviteDeclined,

    /// Invitation was canceled
    #[db_rename = "member_invite_canceled"]
    #[serde(rename = "member_invite_canceled")]
    MemberInviteCanceled,

    // Integration activities
    /// Integration was created
    #[db_rename = "integration_created"]
    #[serde(rename = "integration_created")]
    IntegrationCreated,

    /// Integration was updated
    #[db_rename = "integration_updated"]
    #[serde(rename = "integration_updated")]
    IntegrationUpdated,

    /// Integration was deleted
    #[db_rename = "integration_deleted"]
    #[serde(rename = "integration_deleted")]
    IntegrationDeleted,

    /// Integration was enabled
    #[db_rename = "integration_enabled"]
    #[serde(rename = "integration_enabled")]
    IntegrationEnabled,

    /// Integration was disabled
    #[db_rename = "integration_disabled"]
    #[serde(rename = "integration_disabled")]
    IntegrationDisabled,

    /// Integration completed synchronization
    #[db_rename = "integration_synced"]
    #[serde(rename = "integration_synced")]
    IntegrationSynced,

    /// Integration succeeded
    #[db_rename = "integration_succeeded"]
    #[serde(rename = "integration_succeeded")]
    IntegrationSucceeded,

    /// Integration encountered a failure
    #[db_rename = "integration_failed"]
    #[serde(rename = "integration_failed")]
    IntegrationFailed,

    // Document activities
    /// Document was created
    #[db_rename = "document_created"]
    #[serde(rename = "document_created")]
    DocumentCreated,

    /// Document was updated
    #[db_rename = "document_updated"]
    #[serde(rename = "document_updated")]
    DocumentUpdated,

    /// Document was deleted
    #[db_rename = "document_deleted"]
    #[serde(rename = "document_deleted")]
    DocumentDeleted,

    /// Document was processed
    #[db_rename = "document_processed"]
    #[serde(rename = "document_processed")]
    DocumentProcessed,

    /// Document file was uploaded
    #[db_rename = "document_uploaded"]
    #[serde(rename = "document_uploaded")]
    DocumentUploaded,

    /// Document was downloaded
    #[db_rename = "document_downloaded"]
    #[serde(rename = "document_downloaded")]
    DocumentDownloaded,

    /// Document was verified
    #[db_rename = "document_verified"]
    #[serde(rename = "document_verified")]
    DocumentVerified,

    // Comment activities
    /// Comment was added
    #[db_rename = "comment_added"]
    #[serde(rename = "comment_added")]
    CommentAdded,

    /// Comment was updated
    #[db_rename = "comment_updated"]
    #[serde(rename = "comment_updated")]
    CommentUpdated,

    /// Comment was deleted
    #[db_rename = "comment_deleted"]
    #[serde(rename = "comment_deleted")]
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
    /// Document-related activities
    Document,
    /// Comment-related activities
    Comment,
    /// Custom activities
    Custom,
}
