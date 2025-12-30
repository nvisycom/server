//! Core authorization types and utilities.
//!
//! This module provides the fundamental types used for authorization throughout
//! the nvisy system, including permissions, contexts, and results.

use std::borrow::Cow;

use nvisy_postgres::model::WorkspaceMember;
use nvisy_postgres::types::WorkspaceRole;
use strum::{EnumIter, EnumString, IntoEnumIterator};

use crate::handler::{ErrorKind, Result};

/// Granular workspace permissions for authorization checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(EnumIter, EnumString)]
pub enum Permission {
    // Workspace-level permissions
    /// Can view workspace basic information.
    ViewWorkspace,
    /// Can update workspace settings and metadata.
    UpdateWorkspace,
    /// Can delete the entire workspace.
    DeleteWorkspace,

    // Document permissions
    /// Can view and read documents in the workspace.
    ViewDocuments,
    /// Can create new documents in the workspace.
    CreateDocuments,
    /// Can edit existing documents.
    UpdateDocuments,
    /// Can delete documents from the workspace.
    DeleteDocuments,

    // File and asset permissions
    /// Can view and download files.
    ViewFiles,
    /// Can upload new files to the workspace.
    UploadFiles,
    /// Can update file metadata and properties.
    UpdateFiles,
    /// Can download files from the workspace.
    DownloadFiles,
    /// Can delete files from the workspace.
    DeleteFiles,

    // Member management permissions
    /// Can view workspace members and their roles.
    ViewMembers,
    /// Can invite new members to the workspace.
    InviteMembers,
    /// Can remove members from the workspace.
    RemoveMembers,
    /// Can change member roles and permissions.
    ManageRoles,

    // Integration permissions
    /// Can view workspace integrations.
    ViewIntegrations,
    /// Can create, modify, and manage workspace integrations.
    ManageIntegrations,

    // Workspace settings and configuration
    /// Can view workspace settings.
    ViewSettings,
    /// Can modify workspace settings and configuration.
    ManageSettings,
}

impl Permission {
    /// Checks if the given workspace role satisfies this permission requirement.
    ///
    /// This method leverages the role hierarchy to determine if the given role
    /// has sufficient permissions. A role is permitted if it has equal or higher
    /// permission level than the minimum required role for this permission.
    pub const fn is_permitted_by_role(self, role: WorkspaceRole) -> bool {
        role.has_permission_level_of(self.minimum_required_role())
    }

    /// Returns the minimum role required for this permission.
    #[must_use]
    pub const fn minimum_required_role(self) -> WorkspaceRole {
        match self {
            // Viewer-level permissions
            Self::ViewWorkspace
            | Self::ViewDocuments
            | Self::ViewFiles
            | Self::ViewMembers
            | Self::ViewIntegrations
            | Self::ViewSettings => WorkspaceRole::Viewer,

            // Editor-level permissions
            Self::CreateDocuments
            | Self::UpdateDocuments
            | Self::DeleteDocuments
            | Self::UploadFiles
            | Self::UpdateFiles
            | Self::DownloadFiles
            | Self::DeleteFiles => WorkspaceRole::Editor,

            // Admin-level permissions
            Self::UpdateWorkspace
            | Self::InviteMembers
            | Self::RemoveMembers
            | Self::ManageIntegrations
            | Self::ManageSettings => WorkspaceRole::Admin,

            // Admin-only permissions (highest level)
            Self::DeleteWorkspace | Self::ManageRoles => WorkspaceRole::Admin,
        }
    }

    /// Returns all permissions available to the given role.
    pub fn permissions_for_role(role: WorkspaceRole) -> Vec<Self> {
        Self::iter()
            .filter(|perm| perm.is_permitted_by_role(role))
            .collect()
    }
}

/// Result of an authorization check with detailed information.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthResult {
    pub granted: bool,
    pub member: Option<WorkspaceMember>,
    pub reason: Option<Cow<'static, str>>,
}

impl AuthResult {
    /// Creates a granted authorization result without member information.
    pub const fn granted() -> Self {
        Self {
            granted: true,
            member: None,
            reason: None,
        }
    }

    /// Creates a granted authorization result with member information.
    pub const fn granted_with_member(member: WorkspaceMember) -> Self {
        Self {
            granted: true,
            member: Some(member),
            reason: None,
        }
    }

    /// Creates a denied authorization result with a reason.
    pub fn denied(reason: impl Into<Cow<'static, str>>) -> Self {
        Self {
            granted: false,
            member: None,
            reason: Some(reason.into()),
        }
    }

    /// Converts the result to a `Result` type, returning an error if access is denied.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_server::extract::AuthResult;
    /// let result = AuthResult::granted();
    /// assert!(result.into_result().is_ok());
    ///
    /// let result = AuthResult::denied("Access denied");
    /// assert!(result.into_result().is_err());
    /// ```
    pub fn into_result(self) -> Result<Option<WorkspaceMember>> {
        if self.granted {
            Ok(self.member)
        } else {
            let error = match self.reason {
                Some(reason) => ErrorKind::Forbidden.with_context(reason),
                None => ErrorKind::Forbidden.into_error(),
            };
            Err(error)
        }
    }
}
