//! Database constraint violations organized by functional area.
//!
//! This module provides a comprehensive enumeration of all database constraint violations,
//! organized into logical groups for better maintainability.

// Account-related constraint modules
mod account_api_tokens;
mod account_identities;
mod account_notifications;
mod accounts;

// Workspace-related constraint modules
mod workspace_activities;
mod workspace_invites;
mod workspace_members;
mod workspace_webhooks;
mod workspaces;

// Document / blob storage constraint modules
mod documents;
mod workspace_blobs;

// Detection-related constraint modules
mod detections;

// Review-related constraint modules
mod workspace_review_comments;
mod workspace_reviews;

mod workspace_connection_syncs;
mod workspace_connections;
mod workspace_policies;
mod workspace_providers;

pub use self::account_api_tokens::AccountApiTokenConstraints;
pub use self::account_identities::AccountIdentityConstraints;
pub use self::account_notifications::AccountNotificationConstraints;
pub use self::accounts::AccountConstraints;
pub use self::detections::WorkspaceDetectionConstraints;
pub use self::documents::WorkspaceDocumentConstraints;
pub use self::workspace_activities::WorkspaceActivitiesConstraints;
pub use self::workspace_blobs::WorkspaceBlobConstraints;
pub use self::workspace_connection_syncs::WorkspaceConnectionSyncConstraints;
pub use self::workspace_connections::WorkspaceConnectionConstraints;
pub use self::workspace_invites::WorkspaceInviteConstraints;
pub use self::workspace_members::WorkspaceMemberConstraints;
pub use self::workspace_policies::WorkspacePolicyConstraints;
pub use self::workspace_providers::WorkspaceProviderConstraints;
pub use self::workspace_review_comments::WorkspaceReviewCommentConstraints;
pub use self::workspace_reviews::WorkspaceReviewConstraints;
pub use self::workspace_webhooks::WorkspaceWebhookConstraints;
pub use self::workspaces::WorkspaceConstraints;

/// Declares [`ConstraintViolation`] from one list of `Variant(Enum)` pairs, so
/// the enum and its name parser cannot drift apart.
macro_rules! constraint_violations {
    ($($variant:ident($ty:ty)),+ $(,)?) => {
        /// Any recognised database constraint violation, grouped by table.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ConstraintViolation {
            $($variant($ty),)+
        }

        impl ConstraintViolation {
            /// Parses a violated constraint's name, or `None` if it is not one
            /// the application recognises.
            ///
            /// # Examples
            ///
            /// ```
            /// use nvisy_postgres::types::ConstraintViolation;
            ///
            /// assert!(ConstraintViolation::new("accounts_email_format").is_some());
            /// assert!(ConstraintViolation::new("unknown_constraint").is_none());
            /// ```
            #[must_use]
            pub fn new(constraint: &str) -> Option<Self> {
                // Every per-table enum matches the full constraint name via
                // strum, so each is tried in turn until one parses.
                None$(.or_else(|| constraint.parse().ok().map(Self::$variant)))+
            }
        }
    };
}

constraint_violations! {
    Account(AccountConstraints),
    AccountIdentity(AccountIdentityConstraints),
    AccountNotification(AccountNotificationConstraints),
    AccountApiToken(AccountApiTokenConstraints),
    Workspace(WorkspaceConstraints),
    WorkspaceMember(WorkspaceMemberConstraints),
    WorkspaceInvite(WorkspaceInviteConstraints),
    WorkspaceActivityLog(WorkspaceActivitiesConstraints),
    WorkspaceWebhook(WorkspaceWebhookConstraints),
    WorkspaceDocument(WorkspaceDocumentConstraints),
    WorkspaceBlob(WorkspaceBlobConstraints),
    WorkspaceReview(WorkspaceReviewConstraints),
    WorkspaceReviewComment(WorkspaceReviewCommentConstraints),
    WorkspaceDetection(WorkspaceDetectionConstraints),
    WorkspaceConnection(WorkspaceConnectionConstraints),
    WorkspaceConnectionSync(WorkspaceConnectionSyncConstraints),
    WorkspacePolicy(WorkspacePolicyConstraints),
    WorkspaceProvider(WorkspaceProviderConstraints),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_parsing() {
        assert_eq!(
            ConstraintViolation::new("accounts_email_format"),
            Some(ConstraintViolation::Account(
                AccountConstraints::EmailFormat
            ))
        );

        assert_eq!(
            ConstraintViolation::new("workspace_documents_display_name_length"),
            Some(ConstraintViolation::WorkspaceDocument(
                WorkspaceDocumentConstraints::DisplayNameLength
            ))
        );

        assert_eq!(
            ConstraintViolation::new("blobs_file_size_min"),
            Some(ConstraintViolation::WorkspaceBlob(
                WorkspaceBlobConstraints::FileSizeMin
            ))
        );

        // The definition size is checked on the versions table.
        assert_eq!(
            ConstraintViolation::new("workspace_policy_versions_definition_size"),
            Some(ConstraintViolation::WorkspacePolicy(
                WorkspacePolicyConstraints::DefinitionSize
            ))
        );

        assert_eq!(
            ConstraintViolation::new("workspace_providers_display_name_unique_idx"),
            Some(ConstraintViolation::WorkspaceProvider(
                WorkspaceProviderConstraints::NameUnique
            ))
        );

        assert_eq!(ConstraintViolation::new("unknown_constraint"), None);
    }
}
