//! Authorization provider trait for authenticated users.
//!
//! This module defines the [`AuthProvider`] trait that provides authorization
//! methods for checking permissions at different levels: project, document, admin, and self-access.
//! The trait is designed to be implemented by types that represent authenticated users.

use nvisy_postgres::model::ProjectMember;
use nvisy_postgres::query::{DocumentRepository, ProjectMemberRepository};
use nvisy_postgres::{PgConnection, PgError};
use uuid::Uuid;

use super::{AuthResult, Permission, TRACING_TARGET_AUTHORIZATION};
use crate::handler::Result;

/// Authorization provider for authenticated users.
///
/// This trait provides methods for checking and enforcing permissions at various levels.
/// Implementors must provide access to the user's account ID and admin status.
/// All authorization methods have default implementations with comprehensive database verification.
///
/// # Implementation Requirements
///
/// - [`account_id`](Self::account_id): Must return the authenticated user's UUID
/// - [`is_admin`](Self::is_admin): Must return current admin status
///
/// # Authorization Levels
///
/// - **Global Admin**: Bypasses all project-level restrictions
/// - **Project-Level**: Based on membership and role within specific projects
/// - **Document-Level**: Extends project permissions with ownership rules
/// - **Self-Access**: Operations on the user's own account data
pub trait AuthProvider {
    /// Returns the account ID of the authenticated user.
    fn account_id(&self) -> Uuid;

    /// Returns whether the user has global administrator privileges.
    fn is_admin(&self) -> bool;

    /// Checks if a user has permission to access a project.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection
    /// * `project_id` - Project to check access for
    /// * `permission` - Required permission level
    ///
    /// # Returns
    ///
    /// Returns `AuthResult` with grant status and optional member information.
    ///
    /// # Errors
    ///
    /// Returns database errors if queries fail.
    #[allow(async_fn_in_trait)]
    async fn check_project_permission(
        &self,
        conn: &mut PgConnection,
        project_id: Uuid,
        permission: Permission,
    ) -> Result<AuthResult, PgError> {
        // Global administrators bypass project-level permissions
        if self.is_admin() {
            tracing::debug!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                project_id = %project_id,
                permission = ?permission,
                "access granted: global administrator"
            );

            return Ok(AuthResult::granted());
        }

        // Check project membership
        let member =
            ProjectMemberRepository::find_project_member(conn, project_id, self.account_id())
                .await?;

        let Some(member) = member else {
            tracing::warn!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                project_id = %project_id,
                permission = ?permission,
                "access denied: not a project member"
            );

            return Ok(AuthResult::denied("Not a project member"));
        };

        // Check role permission
        if permission.is_permitted_by_role(member.member_role) {
            tracing::debug!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                project_id = %project_id,
                permission = ?permission,
                role = ?member.member_role,
                "Access granted: sufficient role"
            );

            Ok(AuthResult::granted_with_member(member))
        } else {
            tracing::warn!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                project_id = %project_id,
                permission = ?permission,
                role = ?member.member_role,
                "Access denied: insufficient role"
            );

            Ok(AuthResult::denied(format!(
                "Role {member_role:?} insufficient for {permission:?} permission",
                member_role = member.member_role
            )))
        }
    }

    /// Checks if a user has permission to access a document.
    ///
    /// This method resolves the document's project and checks project-level permissions.
    /// Document owners have special privileges for write operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection
    /// * `document_id` - Document to check access for
    /// * `permission` - Required permission level
    ///
    /// # Returns
    ///
    /// Returns `AuthResult` with grant status and optional member information.
    ///
    /// # Errors
    ///
    /// Returns database errors if queries fail.
    #[allow(async_fn_in_trait)]
    async fn check_document_permission(
        &self,
        conn: &mut PgConnection,
        document_id: Uuid,
        permission: Permission,
    ) -> Result<AuthResult, PgError> {
        // Get the document to find its project
        let document = DocumentRepository::find_document_by_id(conn, document_id).await?;

        let Some(document) = document else {
            tracing::warn!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                document_id = %document_id,
                "access denied: document not found"
            );
            return Ok(AuthResult::denied("Document not found"));
        };

        // Document owners have special privileges for destructive operations
        let is_document_owner = document.account_id == self.account_id();
        let requires_ownership = matches!(
            permission,
            Permission::UpdateDocuments | Permission::DeleteDocuments
        );

        if requires_ownership && !is_document_owner && !self.is_admin() {
            // Non-owners need explicit project-level permissions for destructive operations
            return self
                .check_project_permission(conn, document.project_id, permission)
                .await;
        }

        self.check_project_permission(conn, document.project_id, permission)
            .await
    }

    /// Validates that a user can perform an action on their own account.
    ///
    /// # Arguments
    ///
    /// * `target_account_id` - Account ID to check access for
    ///
    /// # Returns
    ///
    /// Returns `AuthResult` with grant status.
    fn check_self_permission(&self, target_account_id: Uuid) -> Result<AuthResult> {
        let is_self_access = self.account_id() == target_account_id;
        let is_admin = self.is_admin();

        if is_self_access || is_admin {
            tracing::debug!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                target_account_id = %target_account_id,
                is_admin = is_admin,
                access_type = if is_self_access { "self" } else { "admin" },
                "self-permission granted"
            );

            Ok(AuthResult::granted())
        } else {
            tracing::warn!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                target_account_id = %target_account_id,
                "self-permission denied: insufficient privileges"
            );

            Ok(AuthResult::denied("Can only access your own account data"))
        }
    }

    /// Validates that a user has global administrative privileges.
    ///
    /// Global administrators can perform any operation within the system,
    /// including cross-project operations and system administration tasks.
    ///
    /// # Returns
    ///
    /// Returns [`AuthResult`] indicating whether admin access is granted.
    fn check_admin_permission(&self) -> Result<AuthResult> {
        if self.is_admin() {
            tracing::debug!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                "global admin permission granted"
            );
            Ok(AuthResult::granted())
        } else {
            tracing::warn!(
                target: TRACING_TARGET_AUTHORIZATION,
                account_id = %self.account_id(),
                "global admin permission denied"
            );

            Ok(AuthResult::denied(
                "Global administrator privileges required",
            ))
        }
    }

    /// Authorizes project access and returns member information on success.
    ///
    /// This convenience method performs authorization and converts the result
    /// into a standard [`Result`] with optional member information.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for verification
    /// * `project_id` - Target project identifier
    /// * `permission` - Required permission level
    ///
    /// # Returns
    ///
    /// Returns [`ProjectMember`] if authorized with member info,
    /// [`Ok(None)`] if authorized without member info (e.g., global admin),
    /// or [`Err`] if access is denied.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Forbidden`] if access is denied, or propagates database errors.
    #[allow(async_fn_in_trait)]
    async fn authorize_project(
        &self,
        conn: &mut PgConnection,
        project_id: Uuid,
        permission: Permission,
    ) -> Result<Option<ProjectMember>> {
        let auth_result = self
            .check_project_permission(conn, project_id, permission)
            .await?;
        auth_result.into_result()
    }

    /// Authorizes document access with ownership and project-level checks.
    ///
    /// This convenience method handles complex document authorization logic:
    /// - Document owners have enhanced privileges for their own documents
    /// - All access requires at least project membership
    /// - Global administrators bypass all restrictions
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for verification
    /// * `document_id` - Target document identifier
    /// * `permission` - Required permission level
    ///
    /// # Returns
    ///
    /// Returns member information if authorized, or error if access denied.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Forbidden`] if access is denied, or propagates database errors.
    #[allow(async_fn_in_trait)]
    async fn authorize_document(
        &self,
        conn: &mut PgConnection,
        document_id: Uuid,
        permission: Permission,
    ) -> Result<Option<ProjectMember>> {
        let auth_result = self
            .check_document_permission(conn, document_id, permission)
            .await?;
        auth_result.into_result()
    }

    /// Authorizes access to account-specific data.
    ///
    /// Users can access their own account data, and global administrators
    /// can access any account data for system administration purposes.
    ///
    /// # Arguments
    ///
    /// * `target_account_id` - Account ID to authorize access for
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Forbidden`] if the user cannot access the target account.
    fn authorize_self(&self, target_account_id: Uuid) -> Result<()> {
        let auth_result = self.check_self_permission(target_account_id)?;
        auth_result.into_result().map(|_| ())
    }

    /// Authorizes global administrator access.
    ///
    /// This method enforces global administrator privileges for system-level
    /// operations that require elevated access across all projects and resources.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Forbidden`] if the user lacks global admin privileges.
    fn authorize_admin(&self) -> Result<()> {
        let auth_result = self.check_admin_permission()?;
        auth_result.into_result().map(|_| ())
    }
}
