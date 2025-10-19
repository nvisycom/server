//! Project status enumeration for project lifecycle management.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the operational status of a project in its lifecycle.
///
/// This enumeration corresponds to the `PROJECT_STATUS` PostgreSQL enum and is used
/// to manage project states from active development through archival and template usage.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ProjectStatus"]
pub enum ProjectStatus {
    /// Project is active and accessible for normal operations
    #[db_rename = "active"]
    #[serde(rename = "active")]
    #[default]
    Active,

    /// Project is archived but remains accessible (read-only)
    #[db_rename = "archived"]
    #[serde(rename = "archived")]
    Archived,

    /// Project is temporarily suspended (access restricted)
    #[db_rename = "suspended"]
    #[serde(rename = "suspended")]
    Suspended,

    /// Project serves as a template for creating new projects
    #[db_rename = "template"]
    #[serde(rename = "template")]
    Template,
}

impl ProjectStatus {
    /// Returns whether the project is accessible for normal operations.
    #[inline]
    pub fn is_accessible(self) -> bool {
        matches!(self, ProjectStatus::Active | ProjectStatus::Archived)
    }

    /// Returns whether the project allows write operations.
    #[inline]
    pub fn allows_writes(self) -> bool {
        matches!(self, ProjectStatus::Active)
    }

    /// Returns whether the project allows read operations.
    #[inline]
    pub fn allows_reads(self) -> bool {
        matches!(
            self,
            ProjectStatus::Active | ProjectStatus::Archived | ProjectStatus::Template
        )
    }

    /// Returns whether the project is in an active development state.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(self, ProjectStatus::Active)
    }

    /// Returns whether the project is archived.
    #[inline]
    pub fn is_archived(self) -> bool {
        matches!(self, ProjectStatus::Archived)
    }

    /// Returns whether the project is suspended.
    #[inline]
    pub fn is_suspended(self) -> bool {
        matches!(self, ProjectStatus::Suspended)
    }

    /// Returns whether the project serves as a template.
    #[inline]
    pub fn is_template(self) -> bool {
        matches!(self, ProjectStatus::Template)
    }

    /// Returns whether the project status can be changed by users.
    #[inline]
    pub fn can_change_status(self) -> bool {
        // All statuses can be changed except suspended (admin-only action)
        !matches!(self, ProjectStatus::Suspended)
    }

    /// Returns whether members can be invited to this project.
    #[inline]
    pub fn allows_invitations(self) -> bool {
        matches!(self, ProjectStatus::Active)
    }

    /// Returns whether new documents can be created in this project.
    #[inline]
    pub fn allows_new_documents(self) -> bool {
        matches!(self, ProjectStatus::Active)
    }

    /// Returns a description of what the project status means.
    #[inline]
    pub fn description(self) -> &'static str {
        match self {
            ProjectStatus::Active => "Project is active and accessible for all operations",
            ProjectStatus::Archived => "Project is archived and accessible in read-only mode",
            ProjectStatus::Suspended => "Project is temporarily suspended with restricted access",
            ProjectStatus::Template => "Project serves as a template for creating new projects",
        }
    }

    /// Returns project statuses that are accessible to regular users.
    pub fn user_accessible_statuses() -> &'static [ProjectStatus] {
        &[
            ProjectStatus::Active,
            ProjectStatus::Archived,
            ProjectStatus::Template,
        ]
    }
}
