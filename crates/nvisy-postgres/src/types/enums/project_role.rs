//! Project role enumeration for member permissions and access control.

use std::cmp;

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the role and permission level of a project member.
///
/// This enumeration corresponds to the `PROJECT_ROLE` PostgreSQL enum and provides
/// hierarchical access control for project members with clearly defined capabilities.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[derive(DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ProjectRole"]
pub enum ProjectRole {
    /// Full control over the project, including deletion and all management aspects
    #[db_rename = "owner"]
    #[serde(rename = "owner")]
    Owner,

    /// Administrative access with project management capabilities, cannot delete project
    #[db_rename = "admin"]
    #[serde(rename = "admin")]
    Admin,

    /// Can edit content and manage files, but cannot manage members or project settings
    #[db_rename = "editor"]
    #[serde(rename = "editor")]
    Editor,

    /// Read-only access to project content
    #[db_rename = "viewer"]
    #[serde(rename = "viewer")]
    Viewer,
}

impl ProjectRole {
    /// Returns whether this role has permission to view project content.
    #[inline]
    pub fn can_view(self) -> bool {
        // All roles can view content
        true
    }

    /// Returns whether this role has permission to edit project content.
    #[inline]
    pub fn can_edit(self) -> bool {
        matches!(
            self,
            ProjectRole::Owner | ProjectRole::Admin | ProjectRole::Editor
        )
    }

    /// Returns whether this role has permission to comment on project content.
    #[inline]
    pub fn can_comment(self) -> bool {
        // All roles except viewer can comment
        !matches!(self, ProjectRole::Viewer)
    }

    /// Returns whether this role has permission to upload and manage files.
    #[inline]
    pub fn can_manage_files(self) -> bool {
        matches!(
            self,
            ProjectRole::Owner | ProjectRole::Admin | ProjectRole::Editor
        )
    }

    /// Returns whether this role has permission to manage project members.
    #[inline]
    pub fn can_manage_members(self) -> bool {
        matches!(self, ProjectRole::Owner | ProjectRole::Admin)
    }

    /// Returns whether this role has permission to modify project settings.
    #[inline]
    pub fn can_manage_settings(self) -> bool {
        matches!(self, ProjectRole::Owner | ProjectRole::Admin)
    }

    /// Returns whether this role has permission to invite new members.
    #[inline]
    pub fn can_invite_members(self) -> bool {
        matches!(self, ProjectRole::Owner | ProjectRole::Admin)
    }

    /// Returns whether this role has permission to remove members.
    #[inline]
    pub fn can_remove_members(self) -> bool {
        matches!(self, ProjectRole::Owner | ProjectRole::Admin)
    }

    /// Returns whether this role has permission to archive the project.
    #[inline]
    pub fn can_archive_project(self) -> bool {
        matches!(self, ProjectRole::Owner | ProjectRole::Admin)
    }

    /// Returns whether this role has permission to delete the project.
    #[inline]
    pub fn can_delete_project(self) -> bool {
        matches!(self, ProjectRole::Owner)
    }

    /// Returns whether this role has administrative privileges.
    #[inline]
    pub fn is_administrator(self) -> bool {
        matches!(self, ProjectRole::Owner | ProjectRole::Admin)
    }

    /// Returns whether this role can modify project templates.
    #[inline]
    pub fn can_manage_templates(self) -> bool {
        matches!(self, ProjectRole::Owner | ProjectRole::Admin)
    }

    /// Returns whether this role can export project data.
    #[inline]
    pub fn can_export_data(self) -> bool {
        matches!(
            self,
            ProjectRole::Owner | ProjectRole::Admin | ProjectRole::Editor
        )
    }

    /// Returns the hierarchical level of this role (higher number = more permissions).
    #[inline]
    pub fn hierarchy_level(self) -> u8 {
        match self {
            ProjectRole::Viewer => 1,
            ProjectRole::Editor => 2,
            ProjectRole::Admin => 3,
            ProjectRole::Owner => 4,
        }
    }

    /// Returns whether this role has equal or higher permissions than the other role.
    #[inline]
    pub fn has_permission_level_of(self, other: ProjectRole) -> bool {
        self.hierarchy_level() >= other.hierarchy_level()
    }

    /// Returns a description of the role's capabilities.
    #[inline]
    pub fn description(self) -> &'static str {
        match self {
            ProjectRole::Owner => {
                "Full control over the project, including deletion and all management aspects"
            }
            ProjectRole::Admin => {
                "Administrative access with project management capabilities, cannot delete project"
            }
            ProjectRole::Editor => {
                "Can edit content and manage files, but cannot manage members or project settings"
            }
            ProjectRole::Viewer => "Read-only access to project content",
        }
    }
}

impl PartialOrd for ProjectRole {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProjectRole {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.hierarchy_level().cmp(&other.hierarchy_level())
    }
}
