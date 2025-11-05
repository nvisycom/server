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

    // Project settings and configuration
    /// Can view project settings.
    ViewSettings,
    /// Can modify project settings and configuration.
    ManageSettings,
}

impl Permission {
    /// Checks if the given project role satisfies this permission requirement.
    pub const fn is_permitted_by_role(self, role: ProjectRole) -> bool {
        use ProjectRole::{Admin, Editor, Owner, Viewer};

        match self {
            // Project-level permissions
            Self::ViewProject => matches!(role, Viewer | Editor | Admin | Owner),
            Self::UpdateProject => matches!(role, Admin | Owner),
            Self::DeleteProject => matches!(role, Owner),

            // Document permissions
            Self::ViewDocuments => matches!(role, Viewer | Editor | Admin | Owner),
            Self::CreateDocuments => matches!(role, Editor | Admin | Owner),
            Self::UpdateDocuments => matches!(role, Editor | Admin | Owner),
            Self::DeleteDocuments => matches!(role, Editor | Admin | Owner),

            // File permissions
            Self::ViewFiles => matches!(role, Viewer | Editor | Admin | Owner),
            Self::UploadFiles => matches!(role, Editor | Admin | Owner),
            Self::DeleteFiles => matches!(role, Editor | Admin | Owner),

            // Member management permissions
            Self::ViewMembers => matches!(role, Viewer | Editor | Admin | Owner),
            Self::InviteMembers => matches!(role, Admin | Owner),
            Self::RemoveMembers => matches!(role, Admin | Owner),
            Self::ManageRoles => matches!(role, Owner),

            // Settings permissions
            Self::ViewSettings => matches!(role, Editor | Admin | Owner),
            Self::ManageSettings => matches!(role, Admin | Owner),
        }
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
            | Self::ViewSettings => ProjectRole::Viewer,

            // Editor-level permissions
            Self::CreateDocuments
            | Self::UpdateDocuments
            | Self::DeleteDocuments
            | Self::UploadFiles
            | Self::DeleteFiles => ProjectRole::Editor,

            // Admin-level permissions
            Self::UpdateProject
            | Self::InviteMembers
            | Self::RemoveMembers
            | Self::ManageSettings => ProjectRole::Admin,

            // Owner-only permissions
            Self::DeleteProject | Self::ManageRoles => ProjectRole::Owner,
        }
    }

    /// Returns true if this is a read-only permission that doesn't modify anything.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(
            self,
            Self::ViewProject
                | Self::ViewDocuments
                | Self::ViewFiles
                | Self::ViewMembers
                | Self::ViewSettings
        )
    }

    /// Returns true if this permission involves writing or modifying content.
    #[must_use]
    pub const fn is_write_operation(self) -> bool {
        matches!(
            self,
            Self::CreateDocuments
                | Self::UpdateDocuments
                | Self::DeleteDocuments
                | Self::UploadFiles
                | Self::DeleteFiles
                | Self::UpdateProject
        )
    }

    /// Returns true if this permission requires admin or owner privileges.
    #[must_use]
    pub const fn is_admin_only(self) -> bool {
        matches!(
            self,
            Self::UpdateProject
                | Self::DeleteProject
                | Self::InviteMembers
                | Self::RemoveMembers
                | Self::ManageRoles
                | Self::ManageSettings
        )
    }

    /// Returns true if this permission is owner-exclusive.
    #[must_use]
    pub const fn is_owner_only(self) -> bool {
        matches!(self, Self::DeleteProject | Self::ManageRoles)
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

/// Macro for quick authorization checks in handlers.
///
/// This macro provides a concise way to perform authorization checks with automatic
/// error propagation. The authorization methods are called on the underlying `AuthClaims`
/// through `AuthState`'s `Deref` implementation.
///
/// # Patterns
///
/// - `authorize!(project: project_id, auth_state, conn, permission)` - Project authorization
/// - `authorize!(document: document_id, auth_state, conn, permission)` - Document authorization
/// - `authorize!(admin: auth_state)` - Admin authorization
/// - `authorize!(self: auth_state, target_id)` - Self authorization
///
/// # Examples
///
/// ```rust,ignore
/// // Authorize project viewing
/// let member = authorize!(project: project_id, auth_state, &mut conn, ProjectPermission::ViewProject);
///
/// // Authorize document creation
/// authorize!(document: document_id, auth_state, &mut conn, ProjectPermission::CreateDocuments);
///
/// // Authorize file upload
/// authorize!(project: project_id, auth_state, &mut conn, ProjectPermission::UploadFiles);
///
/// // Require global admin privileges
/// authorize!(admin: auth_state);
///
/// // Validate self-access for account operations
/// authorize!(self: auth_state, target_user_id);
///
/// // Authorize member management
/// let member = authorize!(project: project_id, auth_state, &mut conn, ProjectPermission::ManageRoles,);
/// ```
#[macro_export]
macro_rules! authorize {
    // Project authorization
    (project: $project_id:expr, $auth_state:expr, $conn:expr, $permission:expr $(,)?) => {
        $auth_state
            .authorize_project($conn, $project_id, $permission)
            .await?
    };

    // Document authorization
    (document: $document_id:expr, $auth_state:expr, $conn:expr, $permission:expr $(,)?) => {
        $auth_state
            .authorize_document($conn, $document_id, $permission)
            .await?
    };

    // Self authorization
    (self: $auth_state:expr, $target_id:expr $(,)?) => {
        $auth_state.authorize_self($target_id)?
    };

    // Admin authorization
    (admin: $auth_state:expr $(,)?) => {
        $auth_state.authorize_admin()?
    };
}
