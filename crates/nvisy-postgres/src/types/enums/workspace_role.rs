//! Workspace role enumeration for member permissions and access control.

use std::cmp;

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the role and permission level of a workspace member.
///
/// This enumeration corresponds to the `PROJECT_ROLE` PostgreSQL enum and provides
/// hierarchical access control for workspace members with clearly defined capabilities.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::WorkspaceRole"]
pub enum WorkspaceRole {
    /// Administrative access with full workspace management capabilities
    #[db_rename = "admin"]
    #[serde(rename = "admin")]
    Admin,

    /// Can edit content and manage files, but cannot manage members or workspace settings
    #[db_rename = "editor"]
    #[serde(rename = "editor")]
    Editor,

    /// Read-only access to workspace content
    #[db_rename = "viewer"]
    #[serde(rename = "viewer")]
    #[default]
    Viewer,
}

impl WorkspaceRole {
    /// Returns whether this role has administrative privileges.
    #[inline]
    pub fn is_administrator(self) -> bool {
        matches!(self, WorkspaceRole::Admin)
    }

    /// Returns the hierarchical level of this role (higher number = more permissions).
    #[inline]
    pub const fn hierarchy_level(self) -> u8 {
        match self {
            WorkspaceRole::Viewer => 1,
            WorkspaceRole::Editor => 2,
            WorkspaceRole::Admin => 3,
        }
    }

    /// Returns whether this role has equal or higher permissions than the other role.
    #[inline]
    pub const fn has_permission_level_of(self, other: WorkspaceRole) -> bool {
        self.hierarchy_level() >= other.hierarchy_level()
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
