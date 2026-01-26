//! Pipeline runs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Pipeline runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum PipelineRunConstraints {
    // Pipeline run input/output config constraints
    #[strum(serialize = "pipeline_runs_input_config_size")]
    InputConfigSize,
    #[strum(serialize = "pipeline_runs_output_config_size")]
    OutputConfigSize,

    // Pipeline run definition snapshot constraints
    #[strum(serialize = "pipeline_runs_definition_snapshot_size")]
    DefinitionSnapshotSize,

    // Pipeline run error constraints
    #[strum(serialize = "pipeline_runs_error_size")]
    ErrorSize,

    // Pipeline run metrics constraints
    #[strum(serialize = "pipeline_runs_metrics_size")]
    MetricsSize,

    // Pipeline run chronological constraints
    #[strum(serialize = "pipeline_runs_started_after_created")]
    StartedAfterCreated,
    #[strum(serialize = "pipeline_runs_completed_after_started")]
    CompletedAfterStarted,
}

impl PipelineRunConstraints {
    /// Creates a new [`PipelineRunConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            PipelineRunConstraints::InputConfigSize
            | PipelineRunConstraints::OutputConfigSize
            | PipelineRunConstraints::DefinitionSnapshotSize
            | PipelineRunConstraints::ErrorSize
            | PipelineRunConstraints::MetricsSize => ConstraintCategory::Validation,

            PipelineRunConstraints::StartedAfterCreated
            | PipelineRunConstraints::CompletedAfterStarted => ConstraintCategory::Chronological,
        }
    }
}

impl From<PipelineRunConstraints> for String {
    #[inline]
    fn from(val: PipelineRunConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for PipelineRunConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
