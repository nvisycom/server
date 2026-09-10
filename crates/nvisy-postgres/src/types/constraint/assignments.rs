//! Workspace assignments table constraint violations.

use strum::EnumString;

/// Workspace assignments table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceAssignmentConstraints {
    /// A reviewer is assigned a given file at most once.
    #[strum(serialize = "workspace_assignments_file_assignee_key")]
    FileAssigneeUnique,
}
