//! Workspace activities table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace activities table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceActivitiesConstraints {
    // Activity validation constraints
    #[strum(serialize = "workspace_activities_description_length_max")]
    DescriptionLengthMax,
    #[strum(serialize = "workspace_activities_metadata_size")]
    MetadataSize,
}

impl WorkspaceActivitiesConstraints {
    /// Creates a new [`WorkspaceActivitiesConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceActivitiesConstraints::DescriptionLengthMax
            | WorkspaceActivitiesConstraints::MetadataSize => ConstraintCategory::Validation,
        }
    }
}

impl From<WorkspaceActivitiesConstraints> for String {
    #[inline]
    fn from(val: WorkspaceActivitiesConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceActivitiesConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
