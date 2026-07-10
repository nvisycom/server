//! Pipeline runs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Pipeline runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum PipelineRunConstraints {
    // Size constraints
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

impl PipelineRunConstraints {
    /// Creates a new [`PipelineRunConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            PipelineRunConstraints::AnalyzedDocumentKeyLength
            | PipelineRunConstraints::MetadataSize
            | PipelineRunConstraints::IdempotencyKeyLength => ConstraintCategory::Validation,

            PipelineRunConstraints::CompletedAfterStarted => ConstraintCategory::Chronological,
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
