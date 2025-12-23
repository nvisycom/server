//! Project runs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Project runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectRunConstraints {
    // Run identity constraints
    #[strum(serialize = "project_runs_run_name_length")]
    RunNameLength,
    #[strum(serialize = "project_runs_run_type_format")]
    RunTypeFormat,

    // Run timing constraints
    #[strum(serialize = "project_runs_duration_positive")]
    DurationPositive,
    #[strum(serialize = "project_runs_completed_after_started")]
    CompletedAfterStarted,

    // Run data constraints
    #[strum(serialize = "project_runs_result_summary_length")]
    ResultSummaryLength,
    #[strum(serialize = "project_runs_metadata_size")]
    MetadataSize,
    #[strum(serialize = "project_runs_error_details_size")]
    ErrorDetailsSize,

    // Run chronological constraints
    #[strum(serialize = "project_runs_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "project_runs_started_after_created")]
    StartedAfterCreated,
}

impl ProjectRunConstraints {
    /// Creates a new [`ProjectRunConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectRunConstraints::RunNameLength
            | ProjectRunConstraints::RunTypeFormat
            | ProjectRunConstraints::DurationPositive
            | ProjectRunConstraints::ResultSummaryLength
            | ProjectRunConstraints::MetadataSize
            | ProjectRunConstraints::ErrorDetailsSize => ConstraintCategory::Validation,

            ProjectRunConstraints::CompletedAfterStarted
            | ProjectRunConstraints::UpdatedAfterCreated
            | ProjectRunConstraints::StartedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<ProjectRunConstraints> for String {
    #[inline]
    fn from(val: ProjectRunConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectRunConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
