//! Pipeline runs table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Pipeline runs table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePipelineRunConstraints {
    #[strum(serialize = "workspace_pipeline_runs_metadata_size")]
    MetadataSize,
    #[strum(serialize = "workspace_pipeline_runs_idempotency_key_length")]
    IdempotencyKeyLength,
    #[strum(serialize = "workspace_pipeline_runs_idempotency_idx")]
    IdempotencyUnique,
}

impl WorkspacePipelineRunConstraints {
    /// Creates a new [`WorkspacePipelineRunConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
