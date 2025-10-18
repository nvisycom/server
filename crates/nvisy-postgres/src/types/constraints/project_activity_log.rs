//! Project activity log table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::ConstraintCategory;

/// Project activity log table constraint violations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumString, Display, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectActivityLogConstraints {
    // Activity log validation constraints
    #[strum(serialize = "project_activity_log_activity_type_not_empty")]
    ActivityTypeNotEmpty,
    #[strum(serialize = "project_activity_log_activity_type_length_max")]
    ActivityTypeLengthMax,
    #[strum(serialize = "project_activity_log_activity_data_size_min")]
    ActivityDataSizeMin,
    #[strum(serialize = "project_activity_log_activity_data_size_max")]
    ActivityDataSizeMax,
    #[strum(serialize = "project_activity_log_entity_type_length_max")]
    EntityTypeLengthMax,
}

impl ProjectActivityLogConstraints {
    /// Creates a new [`ProjectActivityLogConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectActivityLogConstraints::ActivityTypeNotEmpty
            | ProjectActivityLogConstraints::ActivityTypeLengthMax
            | ProjectActivityLogConstraints::ActivityDataSizeMin
            | ProjectActivityLogConstraints::ActivityDataSizeMax
            | ProjectActivityLogConstraints::EntityTypeLengthMax => ConstraintCategory::Validation,
        }
    }
}

impl From<ProjectActivityLogConstraints> for String {
    #[inline]
    fn from(val: ProjectActivityLogConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectActivityLogConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
