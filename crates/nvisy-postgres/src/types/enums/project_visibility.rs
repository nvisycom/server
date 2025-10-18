//! Project visibility enumeration for access control and discovery.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the visibility and discovery settings for a project.
///
/// This enumeration corresponds to the `PROJECT_VISIBILITY` PostgreSQL enum and is used
/// to control how projects can be discovered and accessed by users.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[derive(DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ProjectVisibility"]
pub enum ProjectVisibility {
    /// Project is private and only accessible to members
    #[db_rename = "private"]
    #[serde(rename = "private")]
    #[default]
    Private,

    /// Project can be discovered by anyone (read permissions still apply based on membership)
    #[db_rename = "public"]
    #[serde(rename = "public")]
    Public,
}

impl ProjectVisibility {
    /// Returns whether the project is restricted to members only.
    #[inline]
    pub fn is_private(self) -> bool {
        matches!(self, ProjectVisibility::Private)
    }

    /// Returns whether the project is publicly visible.
    #[inline]
    pub fn is_public(self) -> bool {
        matches!(self, ProjectVisibility::Public)
    }

    /// Returns a description of what the visibility setting means.
    #[inline]
    pub fn description(self) -> &'static str {
        match self {
            ProjectVisibility::Private => "Only project members can access and view this project",
            ProjectVisibility::Public => {
                "Anyone can discover this project, but access depends on project settings"
            }
        }
    }

    /// Returns a user-friendly explanation of the visibility level.
    #[inline]
    pub fn user_explanation(self) -> &'static str {
        match self {
            ProjectVisibility::Private => "Hidden from public view and search results",
            ProjectVisibility::Public => "Visible in search results and project listings",
        }
    }
}
