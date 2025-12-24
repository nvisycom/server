//! Core authorization types and utilities.
//!
//! This module provides the fundamental types used for authorization throughout
//! the nvisy system, including permissions, contexts, and results.

use std::borrow::Cow;

use nvisy_postgres::model::ProjectMember;
use nvisy_postgres::types::ProjectRole;
use strum::{EnumIter, EnumString, IntoEnumIterator};

use crate::handler::{ErrorKind, Result};

/// Granular project permissions for authorization checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(EnumIter, EnumString)]
pub enum Permission {
    // Project-level permissions
    /// Can view project basic information.
    ViewProject,
    /// Can update project settings and metadata.
    UpdateProject,
    /// Can delete the entire project.
    DeleteProject,

    // Document permissions
    /// Can view and read documents in the project.
    ViewDocuments,
    /// Can create new documents in the project.
    CreateDocuments,
    /// Can edit existing documents.
    UpdateDocuments,
    /// Can delete documents from the project.
    DeleteDocuments,

    // File and asset permissions
    /// Can view and download files.
    ViewFiles,
    /// Can upload new files to the project.
    UploadFiles,
    /// Can update file metadata and properties.
    UpdateFiles,
    /// Can download files from the project.
    DownloadFiles,
    /// Can delete files from the project.
    DeleteFiles,

    // Member management permissions
    /// Can view project members and their roles.
    ViewMembers,
    /// Can invite new members to the project.
    InviteMembers,
    /// Can remove members from the project.
    RemoveMembers,
    /// Can change member roles and permissions.
    ManageRoles,

    // Integration permissions
    /// Can view project integrations.
    ViewIntegrations,
    /// Can create, modify, and manage project integrations.
    ManageIntegrations,

    // Pipeline permissions
    /// Can view project pipelines.
    ViewPipelines,
    /// Can create, modify, and manage project pipelines.
    ManagePipelines,

    // Template permissions
    /// Can view project templates.
    ViewTemplates,
    /// Can create, modify, and manage project templates.
    ManageTemplates,

    // Project settings and configuration
    /// Can view project settings.
    ViewSettings,
    /// Can modify project settings and configuration.
    ManageSettings,
}

impl Permission {
    /// Checks if the given project role satisfies this permission requirement.
    ///
    /// This method leverages the role hierarchy to determine if the given role
    /// has sufficient permissions. A role is permitted if it has equal or higher
    /// permission level than the minimum required role for this permission.
    pub const fn is_permitted_by_role(self, role: ProjectRole) -> bool {
        role.has_permission_level_of(self.minimum_required_role())
    }

    /// Returns the minimum role required for this permission.
    #[must_use]
    pub const fn minimum_required_role(self) -> ProjectRole {
        match self {
            // Viewer-level permissions
            Self::ViewProject
            | Self::ViewDocuments
            | Self::ViewFiles
            | Self::ViewMembers
            | Self::ViewIntegrations
            | Self::ViewPipelines
            | Self::ViewTemplates
            | Self::ViewSettings => ProjectRole::Viewer,

            // Editor-level permissions
            Self::CreateDocuments
            | Self::UpdateDocuments
            | Self::DeleteDocuments
            | Self::UploadFiles
            | Self::UpdateFiles
            | Self::DownloadFiles
            | Self::DeleteFiles => ProjectRole::Editor,

            // Admin-level permissions
            Self::UpdateProject
            | Self::InviteMembers
            | Self::RemoveMembers
            | Self::ManageIntegrations
            | Self::ManagePipelines
            | Self::ManageTemplates
            | Self::ManageSettings => ProjectRole::Admin,

            // Admin-only permissions (highest level)
            Self::DeleteProject | Self::ManageRoles => ProjectRole::Admin,
        }
    }

    /// Returns all permissions available to the given role.
    pub fn permissions_for_role(role: ProjectRole) -> Vec<Self> {
        Self::iter()
            .filter(|perm| perm.is_permitted_by_role(role))
            .collect()
    }
}

/// Result of an authorization check with detailed information.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthResult {
    pub granted: bool,
    pub member: Option<ProjectMember>,
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
    pub const fn granted_with_member(member: ProjectMember) -> Self {
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
    /// assert!(result.into_unit_result().is_ok());
    ///
    /// let result = AuthResult::denied("Access denied");
    /// assert!(result.into_unit_result().is_err());
    /// ```
    pub fn into_result(self) -> Result<Option<ProjectMember>> {
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
