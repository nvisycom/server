//! Workspace members table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace members table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceMemberConstraints {
    // Member validation constraints
    #[strum(serialize = "workspace_members_custom_permissions_size")]
    CustomPermissionsSize,
    #[strum(serialize = "workspace_members_show_order_range")]
    ShowOrderRange,

    // Member chronological constraints
    #[strum(serialize = "workspace_members_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_members_last_accessed_after_created")]
    LastAccessedAfterCreated,
}

impl WorkspaceMemberConstraints {
    /// Creates a new [`WorkspaceMemberConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceMemberConstraints::CustomPermissionsSize
            | WorkspaceMemberConstraints::ShowOrderRange => ConstraintCategory::Validation,

            WorkspaceMemberConstraints::UpdatedAfterCreated
            | WorkspaceMemberConstraints::LastAccessedAfterCreated => {
                ConstraintCategory::Chronological
            }
        }
    }
}

impl From<WorkspaceMemberConstraints> for String {
    #[inline]
    fn from(val: WorkspaceMemberConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceMemberConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
