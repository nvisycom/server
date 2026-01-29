//! Pipeline artifacts table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Pipeline artifacts table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum PipelineArtifactConstraints {
    // Metadata validation constraints
    #[strum(serialize = "workspace_pipeline_artifacts_metadata_size")]
    MetadataSize,
}

impl PipelineArtifactConstraints {
    /// Creates a new [`PipelineArtifactConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            PipelineArtifactConstraints::MetadataSize => ConstraintCategory::Validation,
        }
    }
}

impl From<PipelineArtifactConstraints> for String {
    #[inline]
    fn from(val: PipelineArtifactConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for PipelineArtifactConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
