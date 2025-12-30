//! Workspace runs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceIntegrationRunConstraints {
    // Run identity constraints
    #[strum(serialize = "workspace_integration_runs_run_name_length")]
    RunNameLength,
    #[strum(serialize = "workspace_integration_runs_run_type_format")]
    RunTypeFormat,

    // Run timing constraints
    #[strum(serialize = "workspace_integration_runs_duration_positive")]
    DurationPositive,
    #[strum(serialize = "workspace_integration_runs_completed_after_started")]
    CompletedAfterStarted,

    // Run data constraints
    #[strum(serialize = "workspace_integration_runs_result_summary_length")]
    ResultSummaryLength,
    #[strum(serialize = "workspace_integration_runs_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_integration_runs_error_details_size")]
    ErrorDetailsSize,

    // Run chronological constraints
    #[strum(serialize = "workspace_integration_runs_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_integration_runs_started_after_created")]
    StartedAfterCreated,
}

impl WorkspaceIntegrationRunConstraints {
    /// Creates a new [`WorkspaceIntegrationRunConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceIntegrationRunConstraints::RunNameLength
            | WorkspaceIntegrationRunConstraints::RunTypeFormat
            | WorkspaceIntegrationRunConstraints::DurationPositive
            | WorkspaceIntegrationRunConstraints::ResultSummaryLength
            | WorkspaceIntegrationRunConstraints::MetadataSize
            | WorkspaceIntegrationRunConstraints::ErrorDetailsSize => ConstraintCategory::Validation,

            WorkspaceIntegrationRunConstraints::CompletedAfterStarted
            | WorkspaceIntegrationRunConstraints::UpdatedAfterCreated
            | WorkspaceIntegrationRunConstraints::StartedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<WorkspaceIntegrationRunConstraints> for String {
    #[inline]
    fn from(val: WorkspaceIntegrationRunConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceIntegrationRunConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
