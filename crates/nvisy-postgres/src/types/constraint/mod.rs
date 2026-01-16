//! Database constraint violations organized by functional area.
//!
//! This module provides a comprehensive enumeration of all database constraint violations,
//! organized into logical groups for better maintainability.

// Account-related constraint modules
mod account_action_tokens;
mod account_api_tokens;
mod account_notifications;
mod accounts;

// Workspace-related constraint modules
mod workspace_activities;
mod workspace_integration_runs;
mod workspace_integrations;
mod workspace_invites;
mod workspace_members;
mod workspace_webhooks;
mod workspaces;

// Document-related constraint modules
mod document_annotations;
mod document_chunks;
mod document_comments;
mod document_files;
mod document_versions;
mod documents;

// Chat-related constraint modules
mod chat_operations;
mod chat_sessions;
mod chat_tool_calls;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use self::account_action_tokens::AccountActionTokenConstraints;
pub use self::account_api_tokens::AccountApiTokenConstraints;
pub use self::account_notifications::AccountNotificationConstraints;
pub use self::accounts::AccountConstraints;
pub use self::chat_operations::ChatOperationConstraints;
pub use self::chat_sessions::ChatSessionConstraints;
pub use self::chat_tool_calls::ChatToolCallConstraints;
pub use self::document_annotations::DocumentAnnotationConstraints;
pub use self::document_chunks::DocumentChunkConstraints;
pub use self::document_comments::DocumentCommentConstraints;
pub use self::document_files::DocumentFileConstraints;
pub use self::document_versions::DocumentVersionConstraints;
pub use self::documents::DocumentConstraints;
pub use self::workspace_activities::WorkspaceActivitiesConstraints;
pub use self::workspace_integration_runs::WorkspaceIntegrationRunConstraints;
pub use self::workspace_integrations::WorkspaceIntegrationConstraints;
pub use self::workspace_invites::WorkspaceInviteConstraints;
pub use self::workspace_members::WorkspaceMemberConstraints;
pub use self::workspace_webhooks::WorkspaceWebhookConstraints;
pub use self::workspaces::WorkspaceConstraints;

/// Unified constraint violation enum that can represent any database constraint.
///
/// This enum wraps all specific constraint types, providing a single interface
/// for handling any constraint violation while maintaining type safety and
/// organizational benefits of the separate modules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum ConstraintViolation {
    // Account-related constraints
    Account(AccountConstraints),
    AccountNotification(AccountNotificationConstraints),
    AccountApiToken(AccountApiTokenConstraints),
    AccountActionToken(AccountActionTokenConstraints),

    // Workspace-related constraints
    Workspace(WorkspaceConstraints),
    WorkspaceMember(WorkspaceMemberConstraints),
    WorkspaceInvite(WorkspaceInviteConstraints),
    WorkspaceActivityLog(WorkspaceActivitiesConstraints),
    WorkspaceIntegration(WorkspaceIntegrationConstraints),
    WorkspaceWebhook(WorkspaceWebhookConstraints),
    WorkspaceIntegrationRun(WorkspaceIntegrationRunConstraints),

    // Document-related constraints
    Document(DocumentConstraints),
    DocumentAnnotation(DocumentAnnotationConstraints),
    DocumentChunk(DocumentChunkConstraints),
    DocumentComment(DocumentCommentConstraints),
    DocumentFile(DocumentFileConstraints),
    DocumentVersion(DocumentVersionConstraints),

    // Chat-related constraints
    ChatSession(ChatSessionConstraints),
    ChatToolCall(ChatToolCallConstraints),
    ChatOperation(ChatOperationConstraints),
}

/// Categories of database constraint violations.
///
/// This enum helps classify constraint violations by their purpose and type,
/// making it easier to handle different categories of errors appropriately.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintCategory {
    /// Data validation constraints (format, length, range checks).
    Validation,
    /// Chronological integrity constraints (timestamp relationships).
    Chronological,
    /// Business logic constraints (domain-specific rules).
    BusinessLogic,
    /// Uniqueness constraints (primary keys, unique indexes).
    Uniqueness,
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
    /// let violation = ConstraintViolation::new("accounts_email_address_unique_idx");
    /// assert!(violation.is_some());
    ///
    /// let unknown = ConstraintViolation::new("unknown_constraint");
    /// assert!(unknown.is_none());
    /// ```
    pub fn new(constraint: &str) -> Option<Self> {
        let prefix = constraint.split('_').next()?;
        macro_rules! try_parse {
            ($($parser:expr => $variant:ident),+ $(,)?) => {
                None$(.or_else(|| $parser(constraint).map(Self::$variant)))+
            };
        }

        match prefix {
            "accounts" => try_parse!(AccountConstraints::new => Account),
            "account" => try_parse! {
                AccountNotificationConstraints::new => AccountNotification,
                AccountApiTokenConstraints::new => AccountApiToken,
                AccountActionTokenConstraints::new => AccountActionToken,
            },
            "workspaces" => try_parse!(WorkspaceConstraints::new => Workspace),
            "workspace" => try_parse! {
                WorkspaceMemberConstraints::new => WorkspaceMember,
                WorkspaceInviteConstraints::new => WorkspaceInvite,
                WorkspaceActivitiesConstraints::new => WorkspaceActivityLog,
                WorkspaceIntegrationConstraints::new => WorkspaceIntegration,
                WorkspaceWebhookConstraints::new => WorkspaceWebhook,
                WorkspaceIntegrationRunConstraints::new => WorkspaceIntegrationRun,
            },
            "documents" => try_parse!(DocumentConstraints::new => Document),
            "document" => try_parse! {
                DocumentAnnotationConstraints::new => DocumentAnnotation,
                DocumentChunkConstraints::new => DocumentChunk,
                DocumentCommentConstraints::new => DocumentComment,
                DocumentFileConstraints::new => DocumentFile,
                DocumentVersionConstraints::new => DocumentVersion,
            },
            "chat" => try_parse! {
                ChatSessionConstraints::new => ChatSession,
                ChatToolCallConstraints::new => ChatToolCall,
                ChatOperationConstraints::new => ChatOperation,
            },
            _ => None,
        }
    }

    /// Returns the table name associated with this constraint.
    ///
    /// This is useful for categorizing errors by the table they affect.
    pub fn table_name(&self) -> &'static str {
        match self {
            // Account-related tables
            ConstraintViolation::Account(_) => "accounts",
            ConstraintViolation::AccountNotification(_) => "account_notifications",
            ConstraintViolation::AccountApiToken(_) => "account_api_tokens",
            ConstraintViolation::AccountActionToken(_) => "account_action_tokens",

            // Workspace-related tables
            ConstraintViolation::Workspace(_) => "workspaces",
            ConstraintViolation::WorkspaceMember(_) => "workspace_members",
            ConstraintViolation::WorkspaceInvite(_) => "workspace_invites",
            ConstraintViolation::WorkspaceActivityLog(_) => "workspace_activities",
            ConstraintViolation::WorkspaceIntegration(_) => "workspace_integrations",
            ConstraintViolation::WorkspaceWebhook(_) => "workspace_webhooks",
            ConstraintViolation::WorkspaceIntegrationRun(_) => "workspace_integration_runs",

            // Document-related tables
            ConstraintViolation::Document(_) => "documents",
            ConstraintViolation::DocumentAnnotation(_) => "document_annotations",
            ConstraintViolation::DocumentChunk(_) => "document_chunks",
            ConstraintViolation::DocumentComment(_) => "document_comments",
            ConstraintViolation::DocumentFile(_) => "document_files",
            ConstraintViolation::DocumentVersion(_) => "document_versions",

            // Chat-related tables
            ConstraintViolation::ChatSession(_) => "chat_sessions",
            ConstraintViolation::ChatToolCall(_) => "chat_tool_calls",
            ConstraintViolation::ChatOperation(_) => "chat_operations",
        }
    }

    /// Returns the functional area this constraint belongs to.
    ///
    /// This groups constraints by their business domain for higher-level categorization.
    pub fn functional_area(&self) -> &'static str {
        // TODO: Implement functional area enumeration.
        match self {
            ConstraintViolation::Account(_)
            | ConstraintViolation::AccountNotification(_)
            | ConstraintViolation::AccountApiToken(_)
            | ConstraintViolation::AccountActionToken(_) => "accounts",

            ConstraintViolation::Workspace(_)
            | ConstraintViolation::WorkspaceMember(_)
            | ConstraintViolation::WorkspaceInvite(_)
            | ConstraintViolation::WorkspaceActivityLog(_)
            | ConstraintViolation::WorkspaceIntegration(_)
            | ConstraintViolation::WorkspaceWebhook(_)
            | ConstraintViolation::WorkspaceIntegrationRun(_) => "workspaces",

            ConstraintViolation::Document(_)
            | ConstraintViolation::DocumentAnnotation(_)
            | ConstraintViolation::DocumentChunk(_)
            | ConstraintViolation::DocumentComment(_)
            | ConstraintViolation::DocumentFile(_)
            | ConstraintViolation::DocumentVersion(_) => "documents",

            ConstraintViolation::ChatSession(_)
            | ConstraintViolation::ChatToolCall(_)
            | ConstraintViolation::ChatOperation(_) => "chat",
        }
    }

    /// Returns the category of this constraint violation.
    ///
    /// This helps categorize errors by their type for better error handling and reporting.
    pub fn constraint_category(&self) -> ConstraintCategory {
        match self {
            ConstraintViolation::Account(c) => c.categorize(),
            ConstraintViolation::AccountNotification(c) => c.categorize(),
            ConstraintViolation::AccountApiToken(c) => c.categorize(),
            ConstraintViolation::AccountActionToken(c) => c.categorize(),

            ConstraintViolation::Workspace(c) => c.categorize(),
            ConstraintViolation::WorkspaceMember(c) => c.categorize(),
            ConstraintViolation::WorkspaceInvite(c) => c.categorize(),
            ConstraintViolation::WorkspaceActivityLog(c) => c.categorize(),
            ConstraintViolation::WorkspaceIntegration(c) => c.categorize(),
            ConstraintViolation::WorkspaceWebhook(c) => c.categorize(),
            ConstraintViolation::WorkspaceIntegrationRun(c) => c.categorize(),

            ConstraintViolation::Document(c) => c.categorize(),
            ConstraintViolation::DocumentAnnotation(c) => c.categorize(),
            ConstraintViolation::DocumentChunk(c) => c.categorize(),
            ConstraintViolation::DocumentComment(c) => c.categorize(),
            ConstraintViolation::DocumentFile(c) => c.categorize(),
            ConstraintViolation::DocumentVersion(c) => c.categorize(),

            ConstraintViolation::ChatSession(c) => c.categorize(),
            ConstraintViolation::ChatToolCall(c) => c.categorize(),
            ConstraintViolation::ChatOperation(c) => c.categorize(),
        }
    }

    /// Returns the underlying constraint name as used in the database.
    #[inline]
    pub fn constraint_name(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for ConstraintViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintViolation::Account(c) => write!(f, "{}", c),
            ConstraintViolation::AccountNotification(c) => write!(f, "{}", c),
            ConstraintViolation::AccountApiToken(c) => write!(f, "{}", c),
            ConstraintViolation::AccountActionToken(c) => write!(f, "{}", c),

            ConstraintViolation::Workspace(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceMember(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceInvite(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceActivityLog(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceIntegration(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceWebhook(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceIntegrationRun(c) => write!(f, "{}", c),

            ConstraintViolation::Document(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentAnnotation(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentChunk(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentComment(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentFile(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentVersion(c) => write!(f, "{}", c),

            ConstraintViolation::ChatSession(c) => write!(f, "{}", c),
            ConstraintViolation::ChatToolCall(c) => write!(f, "{}", c),
            ConstraintViolation::ChatOperation(c) => write!(f, "{}", c),
        }
    }
}

impl From<ConstraintViolation> for String {
    #[inline]
    fn from(val: ConstraintViolation) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ConstraintViolation {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value).ok_or_else(|| format!("Unknown constraint: {}", value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_parsing() {
        assert_eq!(
            ConstraintViolation::new("accounts_email_address_unique_idx"),
            Some(ConstraintViolation::Account(
                AccountConstraints::EmailAddressUnique
            ))
        );

        assert_eq!(
            ConstraintViolation::new("document_versions_version_number_min"),
            Some(ConstraintViolation::DocumentVersion(
                DocumentVersionConstraints::VersionNumberMin
            ))
        );

        assert_eq!(ConstraintViolation::new("unknown_constraint"), None);
    }

    #[test]
    fn test_table_name_extraction() {
        let violation = ConstraintViolation::Account(AccountConstraints::EmailAddressUnique);
        assert_eq!(violation.table_name(), "accounts");

        let violation = ConstraintViolation::Workspace(WorkspaceConstraints::DisplayNameLength);
        assert_eq!(violation.table_name(), "workspaces");

        let violation =
            ConstraintViolation::DocumentFile(DocumentFileConstraints::StoragePathNotEmpty);
        assert_eq!(violation.table_name(), "document_files");
    }

    #[test]
    fn test_functional_area_extraction() {
        let violation = ConstraintViolation::Account(AccountConstraints::EmailAddressUnique);
        assert_eq!(violation.functional_area(), "accounts");

        let violation =
            ConstraintViolation::AccountApiToken(AccountApiTokenConstraints::AccessSeqUnique);
        assert_eq!(violation.functional_area(), "accounts");

        let violation =
            ConstraintViolation::WorkspaceMember(WorkspaceMemberConstraints::ShowOrderRange);
        assert_eq!(violation.functional_area(), "workspaces");

        let violation =
            ConstraintViolation::DocumentVersion(DocumentVersionConstraints::VersionNumberMin);
        assert_eq!(violation.functional_area(), "documents");
    }

    #[test]
    fn test_constraint_categorization() {
        let violation = ConstraintViolation::Account(AccountConstraints::DisplayNameLength);
        assert_eq!(
            violation.constraint_category(),
            ConstraintCategory::Validation
        );

        let violation = ConstraintViolation::Account(AccountConstraints::UpdatedAfterCreated);
        assert_eq!(
            violation.constraint_category(),
            ConstraintCategory::Chronological
        );
    }

    #[test]
    fn test_constraint_name_method() {
        let violation =
            ConstraintViolation::Workspace(WorkspaceConstraints::ActiveStatusNotArchived);
        assert_eq!(
            violation.constraint_name(),
            "workspaces_active_status_not_archived"
        );
    }
}
