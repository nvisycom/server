//! Workspace members table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Workspace members table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceMemberConstraints {
    #[strum(serialize = "workspace_members_pkey")]
    MembershipUnique,
}

impl WorkspaceMemberConstraints {
    /// Creates a new [`WorkspaceMemberConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
