//! Workspace connection syncs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace connection syncs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceConnectionSyncConstraints {
    // Size / validation constraints
    #[strum(serialize = "workspace_connection_syncs_records_synced_non_negative")]
    RecordsSyncedNonNegative,
    #[strum(serialize = "workspace_connection_syncs_attempt_positive")]
    AttemptPositive,
    #[strum(serialize = "workspace_connection_syncs_error_message_length")]
    ErrorMessageLength,
    #[strum(serialize = "workspace_connection_syncs_metadata_size")]
    MetadataSize,

    // Chronological constraints
    #[strum(serialize = "workspace_connection_syncs_completed_after_started")]
    CompletedAfterStarted,

    // Uniqueness constraints
    #[strum(serialize = "workspace_connection_syncs_one_active_idx")]
    OneActivePerConnection,
}

impl WorkspaceConnectionSyncConstraints {
    /// Creates a new [`WorkspaceConnectionSyncConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceConnectionSyncConstraints::RecordsSyncedNonNegative
            | WorkspaceConnectionSyncConstraints::AttemptPositive
            | WorkspaceConnectionSyncConstraints::ErrorMessageLength
            | WorkspaceConnectionSyncConstraints::MetadataSize => ConstraintCategory::Validation,

            WorkspaceConnectionSyncConstraints::CompletedAfterStarted => {
                ConstraintCategory::Chronological
            }

            WorkspaceConnectionSyncConstraints::OneActivePerConnection => {
                ConstraintCategory::Uniqueness
            }
        }
    }
}

impl From<WorkspaceConnectionSyncConstraints> for String {
    #[inline]
    fn from(val: WorkspaceConnectionSyncConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceConnectionSyncConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
