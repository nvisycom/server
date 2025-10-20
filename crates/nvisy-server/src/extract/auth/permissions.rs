//! Core authorization types and utilities.
//!
//! This module provides the fundamental types used for authorization throughout
//! the nvisy system, including permissions, contexts, and results.

use std::borrow::Cow;

use nvisy_postgres::models::ProjectMember;
use nvisy_postgres::types::ProjectRole;
use uuid::Uuid;

use crate::handler::{ErrorKind, Result};

/// Granular project permissions for authorization checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectPermission {
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

impl ProjectPermission {
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
            Self::ViewSettings => matches!(role, Viewer | Editor | Admin | Owner),
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
        const ALL_PERMISSIONS: &[ProjectPermission] = &[
            ProjectPermission::ViewProject,
            ProjectPermission::UpdateProject,
            ProjectPermission::DeleteProject,
            ProjectPermission::ViewDocuments,
            ProjectPermission::CreateDocuments,
            ProjectPermission::UpdateDocuments,
            ProjectPermission::DeleteDocuments,
            ProjectPermission::ViewFiles,
            ProjectPermission::UploadFiles,
            ProjectPermission::DeleteFiles,
            ProjectPermission::ViewMembers,
            ProjectPermission::InviteMembers,
            ProjectPermission::RemoveMembers,
            ProjectPermission::ManageRoles,
            ProjectPermission::ViewSettings,
            ProjectPermission::ManageSettings,
        ];

        // Compile-time assertions to ensure permission hierarchy consistency
        const _: () = {
            // Ensure Owner has all permissions
            assert!(ProjectPermission::ViewProject.is_permitted_by_role(ProjectRole::Owner));
            assert!(ProjectPermission::DeleteProject.is_permitted_by_role(ProjectRole::Owner));
            assert!(ProjectPermission::ManageRoles.is_permitted_by_role(ProjectRole::Owner));

            // Ensure Admin cannot delete projects or manage roles
            assert!(!ProjectPermission::DeleteProject.is_permitted_by_role(ProjectRole::Admin));
            assert!(!ProjectPermission::ManageRoles.is_permitted_by_role(ProjectRole::Admin));

            // Ensure Viewer can only view
            assert!(!ProjectPermission::CreateDocuments.is_permitted_by_role(ProjectRole::Viewer));
            assert!(!ProjectPermission::UpdateDocuments.is_permitted_by_role(ProjectRole::Viewer));
        };

        ALL_PERMISSIONS
            .iter()
            .copied()
            .filter(|perm| perm.is_permitted_by_role(role))
            .collect()
    }

    /// Returns a human-readable description of the permission.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::ViewProject => "View project information",
            Self::UpdateProject => "Update project settings and metadata",
            Self::DeleteProject => "Delete the entire project",
            Self::ViewDocuments => "View and read documents",
            Self::CreateDocuments => "Create new documents",
            Self::UpdateDocuments => "Edit existing documents",
            Self::DeleteDocuments => "Delete documents",
            Self::ViewFiles => "View and download files",
            Self::UploadFiles => "Upload new files",
            Self::DeleteFiles => "Delete files",
            Self::ViewMembers => "View project members and their roles",
            Self::InviteMembers => "Invite new members to the project",
            Self::RemoveMembers => "Remove members from the project",
            Self::ManageRoles => "Change member roles and permissions",
            Self::ViewSettings => "View project settings",
            Self::ManageSettings => "Modify project settings and configuration",
        }
    }
}

/// Authorization context containing user information and permissions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuthContext {
    pub account_id: Uuid,
    pub is_admin: bool,
}

impl AuthContext {
    /// Creates a new authorization context.
    #[must_use]
    pub const fn new(account_id: Uuid, is_admin: bool) -> Self {
        Self {
            account_id,
            is_admin,
        }
    }

    /// Returns true if this context represents a global administrator.
    #[must_use]
    pub const fn is_global_admin(&self) -> bool {
        self.is_admin
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
    /// # use nvisy_server::extract::auth::AuthResult;
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
/// - `authorize!(project: auth_state, conn, project_id, permission)` - Project authorization
/// - `authorize!(document: auth_state, conn, document_id, permission)` - Document authorization
/// - `authorize!(admin: auth_state)` - Admin authorization
/// - `authorize!(self: auth_state, target_id)` - Self authorization
///
/// # Examples
///
/// ```rust,ignore
/// // Authorize project viewing
/// let member = authorize!(project: auth_state, &mut conn, project_id, ProjectPermission::ViewProject);
///
/// // Authorize document creation
/// authorize!(document: auth_state, &mut conn, document_id, ProjectPermission::CreateDocuments);
///
/// // Authorize file upload
/// authorize!(project: auth_state, &mut conn, project_id, ProjectPermission::UploadFiles);
///
/// // Require global admin privileges
/// authorize!(admin: auth_state);
///
/// // Validate self-access for account operations
/// authorize!(self: auth_state, target_user_id);
///
/// // Authorize member management
/// let member = authorize!(project: auth_state, &mut conn, project_id, ProjectPermission::ManageRoles);
/// ```
#[macro_export]
macro_rules! authorize {
    // Project authorization
    (project: $auth_state:expr, $conn:expr, $project_id:expr, $permission:expr) => {
        $auth_state
            .authorize_project($conn, $project_id, $permission)
            .await?
    };

    // Document authorization
    (document: $auth_state:expr, $conn:expr, $document_id:expr, $permission:expr) => {
        $auth_state
            .authorize_document($conn, $document_id, $permission)
            .await?
    };

    // Self authorization
    (self: $auth_state:expr, $target_id:expr) => {
        $auth_state.authorize_self($target_id)?
    };

    // Admin authorization
    (admin: $auth_state:expr) => {
        $auth_state.authorize_admin()?
    };
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::types::ProjectRole;

    use super::*;

    #[test]
    fn test_project_level_permissions() {
        // View project - all roles can view
        assert!(ProjectPermission::ViewProject.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::ViewProject.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::ViewProject.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::ViewProject.is_permitted_by_role(ProjectRole::Owner));

        // Update project - only admin and owner
        assert!(!ProjectPermission::UpdateProject.is_permitted_by_role(ProjectRole::Viewer));
        assert!(!ProjectPermission::UpdateProject.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::UpdateProject.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::UpdateProject.is_permitted_by_role(ProjectRole::Owner));

        // Delete project - only owner
        assert!(!ProjectPermission::DeleteProject.is_permitted_by_role(ProjectRole::Viewer));
        assert!(!ProjectPermission::DeleteProject.is_permitted_by_role(ProjectRole::Editor));
        assert!(!ProjectPermission::DeleteProject.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::DeleteProject.is_permitted_by_role(ProjectRole::Owner));
    }

    #[test]
    fn test_document_permissions() {
        // View documents - all roles can view
        assert!(ProjectPermission::ViewDocuments.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::ViewDocuments.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::ViewDocuments.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::ViewDocuments.is_permitted_by_role(ProjectRole::Owner));

        // Create documents - editor and above
        assert!(!ProjectPermission::CreateDocuments.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::CreateDocuments.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::CreateDocuments.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::CreateDocuments.is_permitted_by_role(ProjectRole::Owner));

        // Update documents - editor and above
        assert!(!ProjectPermission::UpdateDocuments.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::UpdateDocuments.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::UpdateDocuments.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::UpdateDocuments.is_permitted_by_role(ProjectRole::Owner));

        // Delete documents - editor and above
        assert!(!ProjectPermission::DeleteDocuments.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::DeleteDocuments.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::DeleteDocuments.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::DeleteDocuments.is_permitted_by_role(ProjectRole::Owner));
    }

    #[test]
    fn test_file_permissions() {
        // View files - all roles can view
        assert!(ProjectPermission::ViewFiles.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::ViewFiles.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::ViewFiles.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::ViewFiles.is_permitted_by_role(ProjectRole::Owner));

        // Upload files - editor and above
        assert!(!ProjectPermission::UploadFiles.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::UploadFiles.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::UploadFiles.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::UploadFiles.is_permitted_by_role(ProjectRole::Owner));

        // Delete files - editor and above
        assert!(!ProjectPermission::DeleteFiles.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::DeleteFiles.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::DeleteFiles.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::DeleteFiles.is_permitted_by_role(ProjectRole::Owner));
    }

    #[test]
    fn test_member_management_permissions() {
        // View members - all roles can view
        assert!(ProjectPermission::ViewMembers.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::ViewMembers.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::ViewMembers.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::ViewMembers.is_permitted_by_role(ProjectRole::Owner));

        // Invite members - admin and above
        assert!(!ProjectPermission::InviteMembers.is_permitted_by_role(ProjectRole::Viewer));
        assert!(!ProjectPermission::InviteMembers.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::InviteMembers.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::InviteMembers.is_permitted_by_role(ProjectRole::Owner));

        // Remove members - admin and above
        assert!(!ProjectPermission::RemoveMembers.is_permitted_by_role(ProjectRole::Viewer));
        assert!(!ProjectPermission::RemoveMembers.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::RemoveMembers.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::RemoveMembers.is_permitted_by_role(ProjectRole::Owner));

        // Manage roles - only owner
        assert!(!ProjectPermission::ManageRoles.is_permitted_by_role(ProjectRole::Viewer));
        assert!(!ProjectPermission::ManageRoles.is_permitted_by_role(ProjectRole::Editor));
        assert!(!ProjectPermission::ManageRoles.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::ManageRoles.is_permitted_by_role(ProjectRole::Owner));
    }

    #[test]
    fn test_settings_permissions() {
        // View settings - all roles can view
        assert!(ProjectPermission::ViewSettings.is_permitted_by_role(ProjectRole::Viewer));
        assert!(ProjectPermission::ViewSettings.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::ViewSettings.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::ViewSettings.is_permitted_by_role(ProjectRole::Owner));

        // Manage settings - admin and above
        assert!(!ProjectPermission::ManageSettings.is_permitted_by_role(ProjectRole::Viewer));
        assert!(!ProjectPermission::ManageSettings.is_permitted_by_role(ProjectRole::Editor));
        assert!(ProjectPermission::ManageSettings.is_permitted_by_role(ProjectRole::Admin));
        assert!(ProjectPermission::ManageSettings.is_permitted_by_role(ProjectRole::Owner));
    }

    #[test]
    fn test_auth_result_conversion() {
        let granted = AuthResult::granted();
        assert!(granted.into_result().is_ok());

        let denied = AuthResult::denied(Cow::Borrowed("test reason"));
        assert!(denied.into_result().is_err());
    }

    #[test]
    fn test_minimum_required_role() {
        // Viewer permissions
        assert_eq!(
            ProjectPermission::ViewProject.minimum_required_role(),
            ProjectRole::Viewer
        );
        assert_eq!(
            ProjectPermission::ViewDocuments.minimum_required_role(),
            ProjectRole::Viewer
        );

        // Editor permissions
        assert_eq!(
            ProjectPermission::CreateDocuments.minimum_required_role(),
            ProjectRole::Editor
        );
        assert_eq!(
            ProjectPermission::UpdateDocuments.minimum_required_role(),
            ProjectRole::Editor
        );

        // Admin permissions
        assert_eq!(
            ProjectPermission::InviteMembers.minimum_required_role(),
            ProjectRole::Admin
        );
        assert_eq!(
            ProjectPermission::UpdateProject.minimum_required_role(),
            ProjectRole::Admin
        );

        // Owner permissions
        assert_eq!(
            ProjectPermission::ManageRoles.minimum_required_role(),
            ProjectRole::Owner
        );
        assert_eq!(
            ProjectPermission::DeleteProject.minimum_required_role(),
            ProjectRole::Owner
        );
    }

    #[test]
    fn test_permission_categories() {
        // Read-only permissions
        assert!(ProjectPermission::ViewProject.is_read_only());
        assert!(ProjectPermission::ViewDocuments.is_read_only());
        assert!(ProjectPermission::ViewFiles.is_read_only());
        assert!(ProjectPermission::ViewMembers.is_read_only());
        assert!(ProjectPermission::ViewSettings.is_read_only());
        assert!(!ProjectPermission::CreateDocuments.is_read_only());
        assert!(!ProjectPermission::UpdateProject.is_read_only());

        // Write operations
        assert!(ProjectPermission::CreateDocuments.is_write_operation());
        assert!(ProjectPermission::UpdateDocuments.is_write_operation());
        assert!(ProjectPermission::DeleteDocuments.is_write_operation());
        assert!(ProjectPermission::UploadFiles.is_write_operation());
        assert!(ProjectPermission::UpdateProject.is_write_operation());
        assert!(!ProjectPermission::ViewProject.is_write_operation());
        assert!(!ProjectPermission::ViewMembers.is_write_operation());

        // Admin-only permissions
        assert!(ProjectPermission::UpdateProject.is_admin_only());
        assert!(ProjectPermission::DeleteProject.is_admin_only());
        assert!(ProjectPermission::InviteMembers.is_admin_only());
        assert!(ProjectPermission::ManageRoles.is_admin_only());
        assert!(ProjectPermission::ManageSettings.is_admin_only());
        assert!(!ProjectPermission::ViewProject.is_admin_only());
        assert!(!ProjectPermission::CreateDocuments.is_admin_only());

        // Owner-only permissions
        assert!(ProjectPermission::DeleteProject.is_owner_only());
        assert!(ProjectPermission::ManageRoles.is_owner_only());
        assert!(!ProjectPermission::UpdateProject.is_owner_only());
        assert!(!ProjectPermission::InviteMembers.is_owner_only());
    }

    #[test]
    fn test_permissions_for_role() {
        let viewer_perms = ProjectPermission::permissions_for_role(ProjectRole::Viewer);
        assert!(viewer_perms.contains(&ProjectPermission::ViewProject));
        assert!(viewer_perms.contains(&ProjectPermission::ViewDocuments));
        assert!(!viewer_perms.contains(&ProjectPermission::CreateDocuments));
        assert!(!viewer_perms.contains(&ProjectPermission::ManageRoles));

        let editor_perms = ProjectPermission::permissions_for_role(ProjectRole::Editor);
        assert!(editor_perms.contains(&ProjectPermission::ViewProject));
        assert!(editor_perms.contains(&ProjectPermission::CreateDocuments));
        assert!(editor_perms.contains(&ProjectPermission::UpdateDocuments));
        assert!(!editor_perms.contains(&ProjectPermission::InviteMembers));
        assert!(!editor_perms.contains(&ProjectPermission::ManageRoles));

        let admin_perms = ProjectPermission::permissions_for_role(ProjectRole::Admin);
        assert!(admin_perms.contains(&ProjectPermission::ViewProject));
        assert!(admin_perms.contains(&ProjectPermission::CreateDocuments));
        assert!(admin_perms.contains(&ProjectPermission::InviteMembers));
        assert!(admin_perms.contains(&ProjectPermission::ManageSettings));
        assert!(!admin_perms.contains(&ProjectPermission::ManageRoles));
        assert!(!admin_perms.contains(&ProjectPermission::DeleteProject));

        let owner_perms = ProjectPermission::permissions_for_role(ProjectRole::Owner);
        assert!(owner_perms.contains(&ProjectPermission::ViewProject));
        assert!(owner_perms.contains(&ProjectPermission::CreateDocuments));
        assert!(owner_perms.contains(&ProjectPermission::InviteMembers));
        assert!(owner_perms.contains(&ProjectPermission::ManageRoles));
        assert!(owner_perms.contains(&ProjectPermission::DeleteProject));
    }

    #[test]
    fn test_permission_descriptions() {
        assert_eq!(
            ProjectPermission::ViewProject.description(),
            "View project information"
        );
        assert_eq!(
            ProjectPermission::ManageRoles.description(),
            "Change member roles and permissions"
        );
        assert_eq!(
            ProjectPermission::DeleteProject.description(),
            "Delete the entire project"
        );
    }

    // Note: AuthContext conversion and other authorization methods are tested
    // in integration tests where database access is available
}
