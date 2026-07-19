//! Workspace connection runs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace connection runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceConnectionRunConstraints {
    // Size / validation constraints
    #[strum(serialize = "workspace_connection_runs_records_synced_non_negative")]
    RecordsSyncedNonNegative,
    #[strum(serialize = "workspace_connection_runs_error_message_length")]
    ErrorMessageLength,
    #[strum(serialize = "workspace_connection_runs_metadata_size")]
    MetadataSize,

    // Chronological constraints
    #[strum(serialize = "workspace_connection_runs_completed_after_started")]
    CompletedAfterStarted,
}

impl WorkspaceConnectionRunConstraints {
    /// Creates a new [`WorkspaceConnectionRunConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceConnectionRunConstraints::RecordsSyncedNonNegative
            | WorkspaceConnectionRunConstraints::ErrorMessageLength
            | WorkspaceConnectionRunConstraints::MetadataSize => ConstraintCategory::Validation,

            WorkspaceConnectionRunConstraints::CompletedAfterStarted => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<WorkspaceConnectionRunConstraints> for String {
    #[inline]
    fn from(val: WorkspaceConnectionRunConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceConnectionRunConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
