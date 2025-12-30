//! Workspace status enumeration for workspace lifecycle tracking.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the operational status of a workspace in its lifecycle.
///
/// This enumeration corresponds to the `PROJECT_STATUS` PostgreSQL enum and is used
/// to manage workspace states from active development through archival usage.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::WorkspaceStatus"]
pub enum WorkspaceStatus {
    /// Workspace is active and accessible for normal operations
    #[db_rename = "active"]
    #[serde(rename = "active")]
    #[default]
    Active,

    /// Workspace is archived but remains accessible (read-only)
    #[db_rename = "archived"]
    #[serde(rename = "archived")]
    Archived,

    /// Workspace is temporarily suspended (access restricted)
    #[db_rename = "suspended"]
    #[serde(rename = "suspended")]
    Suspended,
}

impl WorkspaceStatus {
    /// Returns whether the workspace is accessible for normal operations.
    #[inline]
    pub fn is_accessible(self) -> bool {
        matches!(self, WorkspaceStatus::Active | WorkspaceStatus::Archived)
    }

    /// Returns whether the workspace allows write operations.
    #[inline]
    pub fn allows_writes(self) -> bool {
        matches!(self, WorkspaceStatus::Active)
    }

    /// Returns whether the workspace allows read operations.
    #[inline]
    pub fn allows_reads(self) -> bool {
        matches!(self, WorkspaceStatus::Active | WorkspaceStatus::Archived)
    }

    /// Returns whether the workspace is in an active development state.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(self, WorkspaceStatus::Active)
    }

    /// Returns whether the workspace is archived.
    #[inline]
    pub fn is_archived(self) -> bool {
        matches!(self, WorkspaceStatus::Archived)
    }

    /// Returns whether the workspace is suspended.
    #[inline]
    pub fn is_suspended(self) -> bool {
        matches!(self, WorkspaceStatus::Suspended)
    }

    /// Returns whether the workspace status can be changed by users.
    #[inline]
    pub fn can_change_status(self) -> bool {
        // All statuses can be changed except suspended (admin-only action)
        !matches!(self, WorkspaceStatus::Suspended)
    }

    /// Returns whether members can be invited to this workspace.
    #[inline]
    pub fn allows_invitations(self) -> bool {
        matches!(self, WorkspaceStatus::Active)
    }

    /// Returns whether new documents can be created in this workspace.
    #[inline]
    pub fn allows_new_documents(self) -> bool {
        matches!(self, WorkspaceStatus::Active)
    }

    /// Returns workspace statuses that are accessible to regular users.
    pub fn user_accessible_statuses() -> &'static [WorkspaceStatus] {
        &[WorkspaceStatus::Active, WorkspaceStatus::Archived]
    }
}
