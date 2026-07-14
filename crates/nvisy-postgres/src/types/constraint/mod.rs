//! Database constraint violations organized by functional area.
//!
//! This module provides a comprehensive enumeration of all database constraint violations,
//! organized into logical groups for better maintainability.

// Account-related constraint modules
mod account_api_tokens;
mod account_notifications;
mod accounts;

// Workspace-related constraint modules
mod workspace_activities;
mod workspace_invites;
mod workspace_members;
mod workspace_webhooks;
mod workspaces;

// File-related constraint modules
mod files;

// Pipeline-related constraint modules
mod pipeline_artifacts;
mod pipeline_references;
mod pipeline_runs;
mod pipelines;

mod workspace_connection_runs;
mod workspace_connections;
mod workspace_contexts;
mod workspace_policies;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use self::account_api_tokens::AccountApiTokenConstraints;
pub use self::account_notifications::AccountNotificationConstraints;
pub use self::accounts::AccountConstraints;
pub use self::files::WorkspaceFileConstraints;
pub use self::pipeline_artifacts::WorkspacePipelineArtifactConstraints;
pub use self::pipeline_references::WorkspacePipelineReferenceConstraints;
pub use self::pipeline_runs::WorkspacePipelineRunConstraints;
pub use self::pipelines::WorkspacePipelineConstraints;
pub use self::workspace_activities::WorkspaceActivitiesConstraints;
pub use self::workspace_connection_runs::WorkspaceConnectionRunConstraints;
pub use self::workspace_connections::WorkspaceConnectionConstraints;
pub use self::workspace_contexts::WorkspaceContextConstraints;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum ConstraintViolation {
    // Account-related constraints
    Account(AccountConstraints),
    AccountNotification(AccountNotificationConstraints),
    AccountApiToken(AccountApiTokenConstraints),

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
    WorkspacePipelineArtifact(WorkspacePipelineArtifactConstraints),
    WorkspacePipelineReference(WorkspacePipelineReferenceConstraints),
    WorkspaceConnection(WorkspaceConnectionConstraints),
    WorkspaceConnectionRun(WorkspaceConnectionRunConstraints),
    WorkspaceContext(WorkspaceContextConstraints),
    WorkspacePolicy(WorkspacePolicyConstraints),
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
    /// let violation = ConstraintViolation::new("accounts_email_format");
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
            },
            "workspaces" => try_parse!(WorkspaceConstraints::new => Workspace),
            // Every workspace-owned table is prefixed `workspace_*`, so all of
            // their constraints dispatch here. strum matches the full name, so
            // the order of these parsers does not matter.
            "workspace" => try_parse! {
                WorkspaceMemberConstraints::new => WorkspaceMember,
                WorkspaceInviteConstraints::new => WorkspaceInvite,
                WorkspaceActivitiesConstraints::new => WorkspaceActivityLog,
                WorkspaceWebhookConstraints::new => WorkspaceWebhook,
                WorkspaceConnectionRunConstraints::new => WorkspaceConnectionRun,
                WorkspaceConnectionConstraints::new => WorkspaceConnection,
                WorkspaceContextConstraints::new => WorkspaceContext,
                WorkspacePolicyConstraints::new => WorkspacePolicy,
                WorkspaceFileConstraints::new => WorkspaceFile,
                WorkspacePipelineRunConstraints::new => WorkspacePipelineRun,
                WorkspacePipelineConstraints::new => WorkspacePipeline,
                WorkspacePipelineArtifactConstraints::new => WorkspacePipelineArtifact,
                WorkspacePipelineReferenceConstraints::new => WorkspacePipelineReference,
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

            // Workspace-related tables
            ConstraintViolation::Workspace(_) => "workspaces",
            ConstraintViolation::WorkspaceMember(_) => "workspace_members",
            ConstraintViolation::WorkspaceInvite(_) => "workspace_invites",
            ConstraintViolation::WorkspaceActivityLog(_) => "workspace_activities",
            ConstraintViolation::WorkspaceWebhook(_) => "workspace_webhooks",

            // File-related tables
            ConstraintViolation::WorkspaceFile(_) => "workspace_files",

            // Pipeline-related tables
            ConstraintViolation::WorkspacePipeline(_) => "workspace_pipelines",
            ConstraintViolation::WorkspacePipelineRun(_) => "workspace_pipeline_runs",
            ConstraintViolation::WorkspacePipelineArtifact(_) => "workspace_pipeline_artifacts",
            ConstraintViolation::WorkspacePipelineReference(_) => "pipeline_references",
            ConstraintViolation::WorkspaceConnection(_) => "workspace_connections",
            ConstraintViolation::WorkspaceConnectionRun(_) => "workspace_connection_runs",
            ConstraintViolation::WorkspaceContext(_) => "workspace_contexts",
            ConstraintViolation::WorkspacePolicy(_) => "workspace_policies",
        }
    }

    /// Returns the functional area this constraint belongs to.
    ///
    /// This groups constraints by their business domain for higher-level categorization.
    pub fn functional_area(&self) -> &'static str {
        match self {
            ConstraintViolation::Account(_)
            | ConstraintViolation::AccountNotification(_)
            | ConstraintViolation::AccountApiToken(_) => "accounts",

            ConstraintViolation::Workspace(_)
            | ConstraintViolation::WorkspaceMember(_)
            | ConstraintViolation::WorkspaceInvite(_)
            | ConstraintViolation::WorkspaceActivityLog(_)
            | ConstraintViolation::WorkspaceWebhook(_) => "workspaces",

            ConstraintViolation::WorkspaceFile(_) => "files",

            ConstraintViolation::WorkspacePipeline(_)
            | ConstraintViolation::WorkspacePipelineRun(_)
            | ConstraintViolation::WorkspacePipelineArtifact(_)
            | ConstraintViolation::WorkspacePipelineReference(_) => "pipelines",

            ConstraintViolation::WorkspaceConnection(_)
            | ConstraintViolation::WorkspaceConnectionRun(_) => "connections",
            ConstraintViolation::WorkspaceContext(_) => "contexts",
            ConstraintViolation::WorkspacePolicy(_) => "policies",
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

            ConstraintViolation::Workspace(c) => c.categorize(),
            ConstraintViolation::WorkspaceMember(c) => c.categorize(),
            ConstraintViolation::WorkspaceInvite(c) => c.categorize(),
            ConstraintViolation::WorkspaceActivityLog(c) => c.categorize(),
            ConstraintViolation::WorkspaceWebhook(c) => c.categorize(),

            ConstraintViolation::WorkspaceFile(c) => c.categorize(),

            ConstraintViolation::WorkspacePipeline(c) => c.categorize(),
            ConstraintViolation::WorkspacePipelineRun(c) => c.categorize(),
            ConstraintViolation::WorkspacePipelineArtifact(c) => c.categorize(),
            ConstraintViolation::WorkspacePipelineReference(c) => c.categorize(),
            ConstraintViolation::WorkspaceConnection(c) => c.categorize(),
            ConstraintViolation::WorkspaceConnectionRun(c) => c.categorize(),
            ConstraintViolation::WorkspaceContext(c) => c.categorize(),
            ConstraintViolation::WorkspacePolicy(c) => c.categorize(),
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

            ConstraintViolation::Workspace(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceMember(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceInvite(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceActivityLog(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceWebhook(c) => write!(f, "{}", c),

            ConstraintViolation::WorkspaceFile(c) => write!(f, "{}", c),

            ConstraintViolation::WorkspacePipeline(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspacePipelineRun(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspacePipelineArtifact(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspacePipelineReference(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceConnection(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceConnectionRun(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspaceContext(c) => write!(f, "{}", c),
            ConstraintViolation::WorkspacePolicy(c) => write!(f, "{}", c),
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
            ConstraintViolation::new("accounts_email_format"),
            Some(ConstraintViolation::Account(
                AccountConstraints::EmailFormat
            ))
        );

        // Workspace-owned tables all share the `workspace_*` prefix and dispatch
        // through the same arm; the file/pipeline enums live there too.
        assert_eq!(
            ConstraintViolation::new("workspace_files_version_number_min"),
            Some(ConstraintViolation::WorkspaceFile(
                WorkspaceFileConstraints::VersionNumberMin
            ))
        );
        assert_eq!(
            ConstraintViolation::new("workspace_pipelines_name_length"),
            Some(ConstraintViolation::WorkspacePipeline(
                WorkspacePipelineConstraints::NameLength
            ))
        );
        assert_eq!(
            ConstraintViolation::new("workspace_policies_workspace_id_id_key"),
            Some(ConstraintViolation::WorkspacePolicy(
                WorkspacePolicyConstraints::WorkspaceIdIdUnique
            ))
        );
        assert_eq!(
            ConstraintViolation::new("workspace_pipeline_runs_pipeline_id_run_number_key"),
            Some(ConstraintViolation::WorkspacePipelineRun(
                WorkspacePipelineRunConstraints::RunNumberUnique
            ))
        );

        assert_eq!(ConstraintViolation::new("unknown_constraint"), None);
    }

    #[test]
    fn test_table_name_extraction() {
        let violation = ConstraintViolation::Account(AccountConstraints::EmailFormat);
        assert_eq!(violation.table_name(), "accounts");

        let violation = ConstraintViolation::Workspace(WorkspaceConstraints::DisplayNameLength);
        assert_eq!(violation.table_name(), "workspaces");

        let violation =
            ConstraintViolation::WorkspaceFile(WorkspaceFileConstraints::StoragePathNotEmpty);
        assert_eq!(violation.table_name(), "workspace_files");

        let violation =
            ConstraintViolation::WorkspacePolicy(WorkspacePolicyConstraints::NameLength);
        assert_eq!(violation.table_name(), "workspace_policies");
    }

    #[test]
    fn test_functional_area_extraction() {
        let violation = ConstraintViolation::Account(AccountConstraints::EmailFormat);
        assert_eq!(violation.functional_area(), "accounts");

        let violation =
            ConstraintViolation::WorkspaceFile(WorkspaceFileConstraints::VersionNumberMin);
        assert_eq!(violation.functional_area(), "files");

        let violation =
            ConstraintViolation::WorkspacePolicy(WorkspacePolicyConstraints::NameLength);
        assert_eq!(violation.functional_area(), "policies");
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

        let violation = ConstraintViolation::WorkspacePipelineRun(
            WorkspacePipelineRunConstraints::RunNumberUnique,
        );
        assert_eq!(
            violation.constraint_category(),
            ConstraintCategory::Uniqueness
        );
    }

    #[test]
    fn test_constraint_name_method() {
        let violation =
            ConstraintViolation::WorkspaceFile(WorkspaceFileConstraints::WorkspaceIdIdUnique);
        assert_eq!(
            violation.constraint_name(),
            "workspace_files_workspace_id_id_key"
        );
    }
}
