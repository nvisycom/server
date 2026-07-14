//! Pipeline artifacts table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Pipeline artifacts table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspacePipelineArtifactConstraints {
    // Metadata validation constraints
    #[strum(serialize = "workspace_pipeline_artifacts_metadata_size")]
    MetadataSize,
}

impl WorkspacePipelineArtifactConstraints {
    /// Creates a new [`WorkspacePipelineArtifactConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspacePipelineArtifactConstraints::MetadataSize => ConstraintCategory::Validation,
        }
    }
}

impl From<WorkspacePipelineArtifactConstraints> for String {
    #[inline]
    fn from(val: WorkspacePipelineArtifactConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspacePipelineArtifactConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
