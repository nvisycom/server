//! Workspace role enumeration for member permissions and access control.

use std::cmp;

use super::db_enum;

db_enum! {
    /// The role and permission level of a workspace member.
    ///
    /// Corresponds to the `WORKSPACE_ROLE` PostgreSQL enum and provides
    /// hierarchical access control for workspace members with clearly defined
    /// capabilities.
    pub enum WorkspaceRole: Default = Reviewer, "crate::schema::sql_types::WorkspaceRole" {
        /// Full workspace ownership and management capabilities.
        Owner = "owner",
        /// Can manage members, integrations, and settings, but cannot delete the
        /// workspace or transfer ownership.
        Admin = "admin",
        /// Can edit content and download original files, but cannot manage members
        /// or workspace settings.
        Editor = "editor",
        /// Can review redacted output and audits, but cannot download original
        /// files.
        Reviewer = "reviewer",
    }
}

impl WorkspaceRole {
    /// Returns the hierarchical level of this role (higher number = more
    /// permissions).
    #[inline]
    pub const fn hierarchy_level(self) -> u8 {
        match self {
            WorkspaceRole::Reviewer => 1,
            WorkspaceRole::Editor => 2,
            WorkspaceRole::Admin => 3,
            WorkspaceRole::Owner => 4,
        }
    }

    /// Returns whether this role has equal or higher permissions than the other
    /// role.
    #[inline]
    pub const fn has_permission_level_of(self, other: WorkspaceRole) -> bool {
        self.hierarchy_level() >= other.hierarchy_level()
    }

    /// Returns whether this is the workspace owner — the single top role that
    /// cannot be removed or demoted except by an ownership transfer.
    #[inline]
    pub const fn is_owner(self) -> bool {
        matches!(self, WorkspaceRole::Owner)
    }
}

impl PartialOrd for WorkspaceRole {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WorkspaceRole {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.hierarchy_level().cmp(&other.hierarchy_level())
    }
}
