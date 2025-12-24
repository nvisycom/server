//! Database constraint violations organized by functional area.
//!
//! This module provides a comprehensive enumeration of all database constraint violations,
//! organized into logical groups for better maintainability.

// Account-related constraint modules
pub mod account_action_tokens;
pub mod account_api_tokens;
pub mod account_notifications;
pub mod accounts;

// Project-related constraint modules
pub mod project_activities;
pub mod project_integrations;
pub mod project_invites;
pub mod project_members;
pub mod project_webhooks;
pub mod projects;

// Document-related constraint modules
pub mod document_annotations;
pub mod document_comments;
pub mod document_files;
pub mod document_versions;
pub mod documents;

// Project run constraint modules
pub mod project_runs;

use std::fmt;

pub use account_action_tokens::AccountActionTokenConstraints;
pub use account_api_tokens::AccountApiTokenConstraints;
// Re-export all constraint types for convenience
pub use account_notifications::AccountNotificationConstraints;
pub use accounts::AccountConstraints;
pub use document_annotations::DocumentAnnotationConstraints;
pub use document_comments::DocumentCommentConstraints;
pub use document_files::DocumentFileConstraints;
pub use document_versions::DocumentVersionConstraints;
pub use documents::DocumentConstraints;
pub use project_activities::ProjectActivitiesConstraints;
pub use project_integrations::ProjectIntegrationConstraints;
pub use project_invites::ProjectInviteConstraints;
pub use project_members::ProjectMemberConstraints;
pub use project_runs::ProjectRunConstraints;
pub use project_webhooks::ProjectWebhookConstraints;
pub use projects::ProjectConstraints;
use serde::{Deserialize, Serialize};

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

    // Project-related constraints
    Project(ProjectConstraints),
    ProjectMember(ProjectMemberConstraints),
    ProjectInvite(ProjectInviteConstraints),
    ProjectActivityLog(ProjectActivitiesConstraints),
    ProjectIntegration(ProjectIntegrationConstraints),
    ProjectWebhook(ProjectWebhookConstraints),
    ProjectRun(ProjectRunConstraints),

    // Document-related constraints
    Document(DocumentConstraints),
    DocumentAnnotation(DocumentAnnotationConstraints),
    DocumentComment(DocumentCommentConstraints),
    DocumentFile(DocumentFileConstraints),
    DocumentVersion(DocumentVersionConstraints),
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
        // Route based on constraint name prefix for optimal performance
        // This avoids unnecessary parsing attempts by checking prefixes first

        if constraint.starts_with("accounts_") {
            if let Some(c) = AccountConstraints::new(constraint) {
                return Some(ConstraintViolation::Account(c));
            }
        } else if constraint.starts_with("account_notifications_") {
            if let Some(c) = AccountNotificationConstraints::new(constraint) {
                return Some(ConstraintViolation::AccountNotification(c));
            }
        } else if constraint.starts_with("account_api_tokens_") {
            if let Some(c) = AccountApiTokenConstraints::new(constraint) {
                return Some(ConstraintViolation::AccountApiToken(c));
            }
        } else if constraint.starts_with("account_action_tokens_") {
            if let Some(c) = AccountActionTokenConstraints::new(constraint) {
                return Some(ConstraintViolation::AccountActionToken(c));
            }
        } else if constraint.starts_with("projects_") {
            if let Some(c) = ProjectConstraints::new(constraint) {
                return Some(ConstraintViolation::Project(c));
            }
        } else if constraint.starts_with("project_members_") {
            if let Some(c) = ProjectMemberConstraints::new(constraint) {
                return Some(ConstraintViolation::ProjectMember(c));
            }
        } else if constraint.starts_with("project_invites_") {
            if let Some(c) = ProjectInviteConstraints::new(constraint) {
                return Some(ConstraintViolation::ProjectInvite(c));
            }
        } else if constraint.starts_with("project_activities_") {
            if let Some(c) = ProjectActivitiesConstraints::new(constraint) {
                return Some(ConstraintViolation::ProjectActivityLog(c));
            }
        } else if constraint.starts_with("project_integrations_") {
            if let Some(c) = ProjectIntegrationConstraints::new(constraint) {
                return Some(ConstraintViolation::ProjectIntegration(c));
            }
        } else if constraint.starts_with("project_webhooks_") {
            if let Some(c) = ProjectWebhookConstraints::new(constraint) {
                return Some(ConstraintViolation::ProjectWebhook(c));
            }
        } else if constraint.starts_with("project_runs_") {
            if let Some(c) = ProjectRunConstraints::new(constraint) {
                return Some(ConstraintViolation::ProjectRun(c));
            }
        } else if constraint.starts_with("documents_") {
            if let Some(c) = DocumentConstraints::new(constraint) {
                return Some(ConstraintViolation::Document(c));
            }
        } else if constraint.starts_with("document_annotations_") {
            if let Some(c) = DocumentAnnotationConstraints::new(constraint) {
                return Some(ConstraintViolation::DocumentAnnotation(c));
            }
        } else if constraint.starts_with("document_comments_") {
            if let Some(c) = DocumentCommentConstraints::new(constraint) {
                return Some(ConstraintViolation::DocumentComment(c));
            }
        } else if constraint.starts_with("document_files_") {
            if let Some(c) = DocumentFileConstraints::new(constraint) {
                return Some(ConstraintViolation::DocumentFile(c));
            }
        } else if constraint.starts_with("document_versions_")
            && let Some(c) = DocumentVersionConstraints::new(constraint)
        {
            return Some(ConstraintViolation::DocumentVersion(c));
        }

        None
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

            // Project-related tables
            ConstraintViolation::Project(_) => "projects",
            ConstraintViolation::ProjectMember(_) => "project_members",
            ConstraintViolation::ProjectInvite(_) => "project_invites",
            ConstraintViolation::ProjectActivityLog(_) => "project_activities",
            ConstraintViolation::ProjectIntegration(_) => "project_integrations",
            ConstraintViolation::ProjectWebhook(_) => "project_webhooks",
            ConstraintViolation::ProjectRun(_) => "project_runs",

            // Document-related tables
            ConstraintViolation::Document(_) => "documents",
            ConstraintViolation::DocumentAnnotation(_) => "document_annotations",
            ConstraintViolation::DocumentComment(_) => "document_comments",
            ConstraintViolation::DocumentFile(_) => "document_files",
            ConstraintViolation::DocumentVersion(_) => "document_versions",
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

            ConstraintViolation::Project(_)
            | ConstraintViolation::ProjectMember(_)
            | ConstraintViolation::ProjectInvite(_)
            | ConstraintViolation::ProjectActivityLog(_)
            | ConstraintViolation::ProjectIntegration(_)
            | ConstraintViolation::ProjectWebhook(_)
            | ConstraintViolation::ProjectRun(_) => "projects",

            ConstraintViolation::Document(_)
            | ConstraintViolation::DocumentAnnotation(_)
            | ConstraintViolation::DocumentComment(_)
            | ConstraintViolation::DocumentFile(_)
            | ConstraintViolation::DocumentVersion(_) => "documents",
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

            ConstraintViolation::Project(c) => c.categorize(),
            ConstraintViolation::ProjectMember(c) => c.categorize(),
            ConstraintViolation::ProjectInvite(c) => c.categorize(),
            ConstraintViolation::ProjectActivityLog(c) => c.categorize(),
            ConstraintViolation::ProjectIntegration(c) => c.categorize(),
            ConstraintViolation::ProjectWebhook(c) => c.categorize(),
            ConstraintViolation::ProjectRun(c) => c.categorize(),

            ConstraintViolation::Document(c) => c.categorize(),
            ConstraintViolation::DocumentAnnotation(c) => c.categorize(),
            ConstraintViolation::DocumentComment(c) => c.categorize(),
            ConstraintViolation::DocumentFile(c) => c.categorize(),
            ConstraintViolation::DocumentVersion(c) => c.categorize(),
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

            ConstraintViolation::Project(c) => write!(f, "{}", c),
            ConstraintViolation::ProjectMember(c) => write!(f, "{}", c),
            ConstraintViolation::ProjectInvite(c) => write!(f, "{}", c),
            ConstraintViolation::ProjectActivityLog(c) => write!(f, "{}", c),
            ConstraintViolation::ProjectIntegration(c) => write!(f, "{}", c),
            ConstraintViolation::ProjectWebhook(c) => write!(f, "{}", c),
            ConstraintViolation::ProjectRun(c) => write!(f, "{}", c),

            ConstraintViolation::Document(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentAnnotation(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentComment(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentFile(c) => write!(f, "{}", c),
            ConstraintViolation::DocumentVersion(c) => write!(f, "{}", c),
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

        let violation = ConstraintViolation::Project(ProjectConstraints::DisplayNameLength);
        assert_eq!(violation.table_name(), "projects");

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
            ConstraintViolation::ProjectMember(ProjectMemberConstraints::ShowOrderRange);
        assert_eq!(violation.functional_area(), "projects");

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
        let violation = ConstraintViolation::Project(ProjectConstraints::ActiveStatusNotArchived);
        assert_eq!(
            violation.constraint_name(),
            "projects_active_status_not_archived"
        );
    }
}
