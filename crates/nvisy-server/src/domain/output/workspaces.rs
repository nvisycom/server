//! Workspace service outputs.

use nvisy_postgres::model::{Workspace, WorkspaceMember};

/// A workspace paired with the acting account's membership in it.
pub struct WorkspaceWithMembership {
    /// The workspace.
    pub workspace: Workspace,
    /// The acting account's membership.
    pub membership: WorkspaceMember,
}
