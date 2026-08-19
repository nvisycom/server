//! Workspace activities table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Workspace activities table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceActivitiesConstraints {
    // Activity validation constraints
    #[strum(serialize = "workspace_activities_params_size")]
    ParamsSize,
}

impl WorkspaceActivitiesConstraints {
    /// Creates a new [`WorkspaceActivitiesConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
