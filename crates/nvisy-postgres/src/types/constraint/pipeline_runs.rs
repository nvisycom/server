//! Pipeline runs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Pipeline runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePipelineRunConstraints {
    // Size / validation constraints
    #[strum(serialize = "workspace_pipeline_runs_analyzed_document_key_length")]
    AnalyzedDocumentKeyLength,
    #[strum(serialize = "workspace_pipeline_runs_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_pipeline_runs_idempotency_key_length")]
    IdempotencyKeyLength,

    // Chronological constraints
    #[strum(serialize = "workspace_pipeline_runs_completed_after_started")]
    CompletedAfterStarted,
}

impl WorkspacePipelineRunConstraints {
    /// Creates a new [`WorkspacePipelineRunConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspacePipelineRunConstraints::AnalyzedDocumentKeyLength
            | WorkspacePipelineRunConstraints::MetadataSize
            | WorkspacePipelineRunConstraints::IdempotencyKeyLength => {
                ConstraintCategory::Validation
            }

            WorkspacePipelineRunConstraints::CompletedAfterStarted => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<WorkspacePipelineRunConstraints> for String {
    #[inline]
    fn from(val: WorkspacePipelineRunConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspacePipelineRunConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
