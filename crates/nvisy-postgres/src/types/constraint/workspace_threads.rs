//! Workspace threads table constraint violations.

use strum::EnumString;

/// Workspace threads table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceThreadConstraints {
    /// The title is empty once trimmed, or longer than the maximum.
    #[strum(serialize = "workspace_threads_display_name_length")]
    DisplayNameLength,
    /// The closed-at and closed-by columns disagree on the closed state.
    #[strum(serialize = "workspace_threads_closed_consistent")]
    ClosedConsistent,
}
