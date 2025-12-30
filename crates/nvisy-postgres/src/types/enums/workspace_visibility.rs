//! Workspace visibility enumeration for access control and discovery.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the visibility and discovery settings for a workspace.
///
/// This enumeration corresponds to the `PROJECT_VISIBILITY` PostgreSQL enum and is used
/// to control how workspaces can be discovered and accessed by users.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::WorkspaceVisibility"]
pub enum WorkspaceVisibility {
    /// Workspace is private and only accessible to members
    #[db_rename = "private"]
    #[serde(rename = "private")]
    #[default]
    Private,

    /// Workspace can be discovered by anyone (read permissions still apply based on membership)
    #[db_rename = "public"]
    #[serde(rename = "public")]
    Public,
}

impl WorkspaceVisibility {
    /// Returns whether the workspace is restricted to members only.
    #[inline]
    pub fn is_private(self) -> bool {
        matches!(self, WorkspaceVisibility::Private)
    }

    /// Returns whether the workspace is publicly visible.
    #[inline]
    pub fn is_public(self) -> bool {
        matches!(self, WorkspaceVisibility::Public)
    }
}
