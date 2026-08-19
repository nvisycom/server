//! Workspace activities table constraint violations.

use strum::EnumString;

/// Workspace activities table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceActivitiesConstraints {
    #[strum(serialize = "workspace_activities_params_size")]
    ParamsSize,
}
