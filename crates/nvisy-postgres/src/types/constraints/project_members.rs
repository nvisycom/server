//! Project members table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Project members table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectMemberConstraints {
    // Member validation constraints
    #[strum(serialize = "project_members_custom_permissions_size_min")]
    CustomPermissionsSizeMin,
    #[strum(serialize = "project_members_custom_permissions_size_max")]
    CustomPermissionsSizeMax,
    #[strum(serialize = "project_members_show_order_min")]
    ShowOrderMin,
    #[strum(serialize = "project_members_show_order_max")]
    ShowOrderMax,

    // Member chronological constraints
    #[strum(serialize = "project_members_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "project_members_last_accessed_after_created")]
    LastAccessedAfterCreated,
}

impl ProjectMemberConstraints {
    /// Creates a new [`ProjectMemberConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectMemberConstraints::CustomPermissionsSizeMin
            | ProjectMemberConstraints::CustomPermissionsSizeMax
            | ProjectMemberConstraints::ShowOrderMin
            | ProjectMemberConstraints::ShowOrderMax => ConstraintCategory::Validation,

            ProjectMemberConstraints::UpdatedAfterCreated
            | ProjectMemberConstraints::LastAccessedAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<ProjectMemberConstraints> for String {
    #[inline]
    fn from(val: ProjectMemberConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectMemberConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
