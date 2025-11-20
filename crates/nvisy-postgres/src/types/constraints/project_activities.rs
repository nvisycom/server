//! Project activities table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Project activities table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectActivitiesConstraints {
    // Activity validation constraints
    #[strum(serialize = "project_activities_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "project_activities_metadata_size")]
    MetadataSize,
}

impl ProjectActivitiesConstraints {
    /// Creates a new [`ProjectActivityLogConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectActivitiesConstraints::DescriptionLengthMax
            | ProjectActivitiesConstraints::MetadataSize => ConstraintCategory::Validation,
        }
    }
}

impl From<ProjectActivitiesConstraints> for String {
    #[inline]
    fn from(val: ProjectActivitiesConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectActivitiesConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
