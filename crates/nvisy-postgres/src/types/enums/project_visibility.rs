//! Project visibility enumeration for access control and discovery.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Defines the visibility and discovery settings for a project.
///
/// This enumeration corresponds to the `PROJECT_VISIBILITY` PostgreSQL enum and is used
/// to control how projects can be discovered and accessed by users.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
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
}
