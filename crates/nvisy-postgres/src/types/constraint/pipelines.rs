//! Pipelines table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Pipelines table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum PipelineConstraints {
    // Pipeline name validation constraints
    #[strum(serialize = "pipelines_name_length")]
    NameLength,

    // Pipeline description validation constraints
    #[strum(serialize = "pipelines_description_length")]
    DescriptionLength,

    // Pipeline definition constraints
    #[strum(serialize = "pipelines_definition_size")]
    DefinitionSize,

    // Pipeline metadata constraints
    #[strum(serialize = "pipelines_metadata_size")]
    MetadataSize,

    // Pipeline chronological constraints
    #[strum(serialize = "pipelines_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "pipelines_deleted_after_created")]
    DeletedAfterCreated,
}

impl PipelineConstraints {
    /// Creates a new [`PipelineConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            PipelineConstraints::NameLength
            | PipelineConstraints::DescriptionLength
            | PipelineConstraints::DefinitionSize
            | PipelineConstraints::MetadataSize => ConstraintCategory::Validation,

            PipelineConstraints::UpdatedAfterCreated | PipelineConstraints::DeletedAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<PipelineConstraints> for String {
    #[inline]
    fn from(val: PipelineConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for PipelineConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
