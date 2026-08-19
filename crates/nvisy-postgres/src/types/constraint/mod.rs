//! Database constraint violations organized by functional area.
//!
//! This module provides a comprehensive enumeration of all database constraint violations,
//! organized into logical groups for better maintainability.

// Account-related constraint modules
mod account_api_tokens;
mod account_notifications;
mod accounts;

// Chat constraint modules
mod chat_messages;
mod chat_sessions;

// Workspace-related constraint modules
mod workspace_activities;
mod workspace_invites;
mod workspace_members;
mod workspace_webhooks;
mod workspaces;

// File-related constraint modules
mod files;

// Pipeline-related constraint modules
mod pipeline_references;
mod pipeline_runs;
mod pipelines;

mod workspace_connection_syncs;
mod workspace_connections;
mod workspace_policies;

pub use self::account_api_tokens::AccountApiTokenConstraints;
pub use self::account_notifications::AccountNotificationConstraints;
pub use self::accounts::AccountConstraints;
pub use self::chat_messages::ChatMessageConstraints;
pub use self::chat_sessions::ChatSessionConstraints;
pub use self::files::WorkspaceFileConstraints;
pub use self::pipeline_references::WorkspacePipelineReferenceConstraints;
pub use self::pipeline_runs::WorkspacePipelineRunConstraints;
pub use self::pipelines::WorkspacePipelineConstraints;
pub use self::workspace_activities::WorkspaceActivitiesConstraints;
pub use self::workspace_connection_syncs::WorkspaceConnectionSyncConstraints;
pub use self::workspace_connections::WorkspaceConnectionConstraints;
pub use self::workspace_invites::WorkspaceInviteConstraints;
pub use self::workspace_members::WorkspaceMemberConstraints;
pub use self::workspace_policies::WorkspacePolicyConstraints;
pub use self::workspace_webhooks::WorkspaceWebhookConstraints;
pub use self::workspaces::WorkspaceConstraints;

/// Unified constraint violation enum that can represent any database constraint.
///
/// This enum wraps all specific constraint types, providing a single interface
/// for handling any constraint violation while maintaining type safety and
/// organizational benefits of the separate modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintViolation {
    // Account-related constraints
    Account(AccountConstraints),
    AccountNotification(AccountNotificationConstraints),
    AccountApiToken(AccountApiTokenConstraints),

    // Chat-related constraints
    ChatSession(ChatSessionConstraints),
    ChatMessage(ChatMessageConstraints),

    // Workspace-related constraints
    Workspace(WorkspaceConstraints),
    WorkspaceMember(WorkspaceMemberConstraints),
    WorkspaceInvite(WorkspaceInviteConstraints),
    WorkspaceActivityLog(WorkspaceActivitiesConstraints),
    WorkspaceWebhook(WorkspaceWebhookConstraints),

    // File-related constraints
    WorkspaceFile(WorkspaceFileConstraints),

    // Pipeline-related constraints
    WorkspacePipeline(WorkspacePipelineConstraints),
    WorkspacePipelineRun(WorkspacePipelineRunConstraints),
    WorkspacePipelineReference(WorkspacePipelineReferenceConstraints),
    WorkspaceConnection(WorkspaceConnectionConstraints),
    WorkspaceConnectionSync(WorkspaceConnectionSyncConstraints),
    WorkspacePolicy(WorkspacePolicyConstraints),
}

impl ConstraintViolation {
    /// Creates a new [`ConstraintViolation`] from the constraint name.
    ///
    /// This method attempts to parse a constraint name string into the corresponding
    /// enum variant. It returns `None` if the constraint name is not recognized.
    ///
    /// # Arguments
    ///
    /// * `constraint` - The name of the database constraint that was violated
    ///
    /// # Returns
    ///
    /// * `Some(ConstraintViolation)` if the constraint name is recognized
    /// * `None` if the constraint name is unknown
    ///
    /// # Examples
    ///
    /// ```
    /// use nvisy_postgres::types::ConstraintViolation;
    ///
    /// let violation = ConstraintViolation::new("accounts_email_format");
    /// assert!(violation.is_some());
    ///
    /// let unknown = ConstraintViolation::new("unknown_constraint");
    /// assert!(unknown.is_none());
    /// ```
    pub fn new(constraint: &str) -> Option<Self> {
        // Every per-table enum matches the full constraint name via strum, so
        // parsing is tried against each in turn until one succeeds.
        macro_rules! try_parse {
            ($($variant:ident),+ $(,)?) => {
                None$(.or_else(|| constraint.parse().ok().map(Self::$variant)))+
            };
        }

        try_parse! {
            Account,
            AccountNotification,
            AccountApiToken,
            ChatSession,
            ChatMessage,
            Workspace,
            WorkspaceMember,
            WorkspaceInvite,
            WorkspaceActivityLog,
            WorkspaceWebhook,
            WorkspaceFile,
            WorkspacePipeline,
            WorkspacePipelineRun,
            WorkspacePipelineReference,
            WorkspaceConnection,
            WorkspaceConnectionSync,
            WorkspacePolicy,
        }
    }
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
            ConstraintViolation::new("workspace_files_version_number_min"),
            Some(ConstraintViolation::WorkspaceFile(
                WorkspaceFileConstraints::VersionNumberMin
            ))
        );

        assert_eq!(ConstraintViolation::new("unknown_constraint"), None);
    }
}
