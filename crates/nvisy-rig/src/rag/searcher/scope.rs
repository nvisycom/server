//! Search scope for restricting vector queries.

use uuid::Uuid;

/// Search scope for vector queries.
///
/// Restricts search to specific files or a workspace to prevent cross-user data access.
#[derive(Debug, Clone)]
pub enum SearchScope {
    /// Search within specific files.
    Files(Vec<Uuid>),

    /// Search within a workspace (all files in that workspace).
    Workspace(Uuid),
}

impl SearchScope {
    /// Creates a scope for a single file.
    pub fn file(file_id: Uuid) -> Self {
        Self::Files(vec![file_id])
    }

    /// Creates a scope for multiple files.
    pub fn files(file_ids: Vec<Uuid>) -> Self {
        Self::Files(file_ids)
    }

    /// Creates a scope for a workspace.
    pub fn workspace(workspace_id: Uuid) -> Self {
        Self::Workspace(workspace_id)
    }
}
