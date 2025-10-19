//! Project member management handlers.
//!
//! This module provides comprehensive project member management functionality,
//! allowing project administrators to view, add, modify, and remove project
//! members. All operations are secured with proper authorization and follow
//! role-based access control principles.
//!
//! # Security Features
//!
//! ## Authorization Requirements
//! - Project membership required for viewing member lists
//! - Admin or owner permissions required for member management operations
//! - Role-based restrictions on permission changes
//! - Self-management restrictions for role modifications
//!
//! ## Role Hierarchy
//! - **Viewer**: Read-only access to project resources
//! - **Editor**: Can create and modify documents within the project
//! - **Admin**: Can manage project settings and members (except owners)
//! - **Owner**: Full control over project and all members
//!
//! ## Member Management Rules
//! - Only admins and owners can manage members
//! - Owners cannot be removed or have roles changed by non-owners
//! - Users cannot modify their own roles
//! - At least one owner must remain in each project
//!
//! # Endpoints
//!
//! ## Member Operations
//! - `GET /projects/{projectId}/members` - List all project members
//! - `GET /projects/{projectId}/members/{memberId}` - Get specific member details
//! - `PUT /projects/{projectId}/members/{memberId}` - Update member role
//! - `DELETE /projects/{projectId}/members/{memberId}` - Remove member from project
//!
//! # Data Privacy
//!
//! - Member email addresses are only visible to project administrators
//! - Member activity logs are restricted based on permissions
//! - Personal information is protected according to privacy policies
//!
//! # Audit and Compliance
//!
//! - All member management operations are logged for audit trails
//! - Role changes include timestamps and administrator attribution
//! - Member removal includes cleanup of associated resources

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgDatabase;
use nvisy_postgres::models::ProjectMember;
use nvisy_postgres::queries::ProjectRepository;
use nvisy_postgres::types::ProjectRole;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{
    AuthState, Json, Path, Path, ProjectPermission, ProjectPermission, ValidateJson,
};
use crate::handler::projects::ProjectPathParams;
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
/// Tracing target for project member operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_members";
use crate::service::ServiceState;

/// Path parameters for member-specific endpoints.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct MemberPathParams {
    /// Unique identifier of the project.
    pub project_id: Uuid,
    /// Unique identifier of the member account.
    pub account_id: Uuid,
}

/// Represents a project member in list responses.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListMembersResponseItem {
    /// Account ID of the member.
    pub account_id: Uuid,
    /// Role of the member in the project.
    pub member_role: ProjectRole,
    /// Whether the member is currently active.
    pub is_active: bool,
    /// Timestamp when the member joined the project.
    pub created_at: OffsetDateTime,
    /// Timestamp when the member last accessed the project.
    pub last_accessed_at: Option<OffsetDateTime>,
}

impl From<ProjectMember> for ListMembersResponseItem {
    fn from(member: ProjectMember) -> Self {
        Self {
            account_id: member.account_id,
            member_role: member.member_role,
            is_active: member.is_active,
            created_at: member.created_at,
            last_accessed_at: member.last_accessed_at,
        }
    }
}

/// Response for listing project members.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListMembersResponse {
    /// ID of the project.
    pub project_id: Uuid,
    /// List of project members.
    pub members: Vec<ListMembersResponseItem>,
}

/// Detailed information about a project member.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct GetMemberResponse {
    /// Account ID of the member.
    pub account_id: Uuid,
    /// Role of the member in the project.
    pub member_role: ProjectRole,
    /// Whether the member is currently active.
    pub is_active: bool,
    /// Whether the member receives update notifications.
    pub notify_updates: bool,
    /// Whether the member receives comment notifications.
    pub notify_comments: bool,
    /// Whether the member receives mention notifications.
    pub notify_mentions: bool,
    /// Whether the project is marked as favorite by this member.
    pub is_favorite: bool,
    /// Timestamp when the member joined the project.
    pub created_at: OffsetDateTime,
    /// Timestamp when the membership was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the member last accessed the project.
    pub last_accessed_at: Option<OffsetDateTime>,
}

impl From<ProjectMember> for GetMemberResponse {
    fn from(member: ProjectMember) -> Self {
        Self {
            account_id: member.account_id,
            member_role: member.member_role,
            is_active: member.is_active,
            notify_updates: member.notify_updates,
            notify_comments: member.notify_comments,
            notify_mentions: member.notify_mentions,
            is_favorite: member.is_favorite,
            created_at: member.created_at,
            updated_at: member.updated_at,
            last_accessed_at: member.last_accessed_at,
        }
    }
}

/// Response for member deletion operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DeleteMemberResponse {
    /// Account ID of the removed member.
    pub account_id: Uuid,
    /// Project ID from which the member was removed.
    pub project_id: Uuid,
}

/// Lists all members of a project.
///
/// Returns a paginated list of project members with their roles and status.
/// Requires administrator permissions to view the member list.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/projects/{projectId}/members/", tag = "members",
    params(ProjectPathParams),
    request_body(
        content = Pagination,
        description = "Pagination parameters",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Access denied - insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Project not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Project members listed successfully",
            body = ListMembersResponse,
        ),
    ),
)]
async fn list_members(
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ListMembersResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "listing project members"
    );

    // Verify user has permission to view project members
    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::ViewMembers,
        )
        .await?;

    // Retrieve project members with pagination
    let project_members = ProjectRepository::list_project_members(
        &mut conn,
        path_params.project_id,
        true, // active members only
        pagination.into(),
    )
    .await?;

    let members = project_members
        .into_iter()
        .map(ListMembersResponseItem::from)
        .collect();

    let response = ListMembersResponse {
        project_id: path_params.project_id,
        members,
    };

    tracing::info!(
        target: "server::handler::project_members",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_count = response.members.len(),
        "project members listed successfully"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Gets detailed information about a specific project member.
///
/// Returns comprehensive information about a project member, including their role,
/// permissions, and activity status. Requires administrator permissions.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/projects/{projectId}/members/{accountId}/", tag = "members",
    params(MemberPathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Access denied - insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Project or member not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Project member retrieved successfully",
            body = GetMemberResponse,
        ),
    )
)]
async fn get_member(
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<(StatusCode, Json<GetMemberResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        "retrieving project member details"
    );

    // Verify user has permission to view project member details
    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::ViewMembers,
        )
        .await?;

    // Find the specific project member
    let project_member = ProjectRepository::find_project_member(
        &mut conn,
        path_params.project_id,
        path_params.account_id,
    )
    .await?
    .ok_or_else(|| ErrorKind::NotFound.with_resource("project member"))?;

    tracing::debug!(
        target: "server::handler::project_members",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        member_role = ?project_member.member_role,
        "project member retrieved successfully"
    );

    Ok((StatusCode::OK, Json(project_member.into())))
}

/// Removes a member from a project.
///
/// Permanently removes a member from the project. This action cannot be undone.
/// The member will lose all access to the project and its resources.
/// Requires administrator permissions.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    delete, path = "/projects/{projectId}/members/{accountId}/", tag = "members",
    params(MemberPathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Access denied - insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Project or member not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Project member removed successfully",
            body = DeleteMemberResponse,
        ),
    )
)]
async fn delete_member(
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<(StatusCode, Json<DeleteMemberResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.member_id.to_string(),
        "Removing project member"
    );

    // Verify user has permission to remove members
    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::RemoveMembers,
        )
        .await?;

    // Prevent users from removing themselves (they should use leave endpoint instead)
    if auth_claims.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest.with_context(
            "Cannot remove yourself from the project. Use the leave project endpoint instead.",
        ));
    }

    // Get the member being removed
    let member_to_remove = ProjectRepository::find_project_member(
        &mut conn,
        path_params.project_id,
        path_params.account_id,
    )
    .await?
    .ok_or_else(|| ErrorKind::NotFound.with_resource("project member"))?;

    // Check if removing the last owner
    if member_to_remove.role == nvisy_postgres::types::ProjectRole::Owner {
        // Count how many active owners exist
        let all_members =
            ProjectRepository::find_all_project_members(&mut conn, path_params.project_id).await?;

        let owner_count = all_members
            .iter()
            .filter(|m| m.role == nvisy_postgres::types::ProjectRole::Owner && m.is_active)
            .count();

        if owner_count <= 1 {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot remove last owner")
                .with_context(
                    "Projects must have at least one owner. Transfer ownership or promote another member to owner before removing this member."
                )
                .into_error());
        }
    }

    // Remove the member from the project
    let success = ProjectRepository::remove_project_member(
        &mut conn,
        path_params.project_id,
        path_params.account_id,
    )
    .await?;

    if !success {
        return Err(ErrorKind::NotFound.with_resource("project member"));
    }

    let response = DeleteMemberResponse {
        account_id: path_params.account_id,
        project_id: path_params.project_id,
    };

    tracing::warn!(
        target: "server::handler::project_members",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        "project member removed successfully"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Request to update a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberRoleRequest {
    /// New role for the member
    pub role: ProjectRole,
}

/// Response after updating a member's role.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberRoleResponse {
    /// Member's account ID
    pub account_id: Uuid,
    /// Project ID
    pub project_id: Uuid,
    /// New role
    pub role: ProjectRole,
    /// When the update occurred
    pub updated_at: OffsetDateTime,
}

/// Updates a project member's role.
///
/// Allows project owners/admins to change a member's permission level.
/// Cannot update your own role. Cannot remove the last owner.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/projects/{projectId}/members/{accountId}/role", tag = "projects",
    params(MemberPathParams),
    request_body = UpdateMemberRoleRequest,
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request - cannot modify own role or remove last owner",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Member not found",
            body = ErrorResponse,
        ),
        (
            status = UNAUTHORIZED,
            description = "Unauthorized - insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Member role updated successfully",
            body = UpdateMemberRoleResponse,
        ),
    )
)]
async fn update_member_role(
    State(pg_database): State<PgDatabase>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<MemberPathParams>,
    ValidateJson(request): ValidateJson<UpdateMemberRoleRequest>,
) -> Result<(StatusCode, Json<UpdateMemberRoleResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.member_id.to_string(),
        new_role = ?request.new_role,
        "updating project member role"
    );

    // Verify user has permission to manage member roles
    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::ManageRoles,
        )
        .await?;

    // Prevent users from updating their own role
    if auth_claims.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot update your own role")
            .with_context("Ask another project owner or admin to update your role")
            .into_error());
    }

    // Get the current member
    // Get current member information
    let current_member = ProjectRepository::find_project_member(
        &mut conn,
        path_params.project_id,
        path_params.account_id,
    )
    .await?
    .ok_or_else(|| ErrorKind::NotFound.with_resource("project member"))?;

    // If demoting from owner, check we're not removing the last owner
    if member.role == ProjectRole::Owner && request.role != ProjectRole::Owner {
        let all_members =
            ProjectRepository::find_all_project_members(&mut conn, path_params.project_id).await?;

        let owner_count = all_members
            .iter()
            .filter(|m| m.role == ProjectRole::Owner && m.is_active)
            .count();

        if owner_count <= 1 {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot demote last owner")
                .with_context(
                    "Projects must have at least one owner. Promote another member to owner first.",
                )
                .into_error());
        }
    }

    // Update the role
    member.role = request.role;
    member.updated_at = OffsetDateTime::now_utc();

    let updated_member = ProjectRepository::update_project_member(&mut conn, member)
        .await
        .map_err(|err| {
            tracing::error!(
                target: "server::handler::project_members",
                error = %err,
                member_id = %path_params.account_id,
                "Failed to update member role"
            );
            ErrorKind::InternalServerError.with_message("Failed to update member role")
        })?;

    tracing::info!(
        target: "server::handler::project_members",
        member_id = %updated_member.account_id,
        project_id = %updated_member.project_id,
        new_role = ?updated_member.role,
        "Member role updated successfully"
    );

    let response = UpdateMemberRoleResponse {
        account_id: updated_member.account_id,
        project_id: updated_member.project_id,
        role: updated_member.role,
        updated_at: updated_member.updated_at,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Returns a [`Router`] with all project member related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new()
        .routes(routes!(list_members))
        .routes(routes!(get_member, update_member_role, delete_member))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn project_member_routes_integration() -> anyhow::Result<()> {
        let _server = create_test_server_with_router(|_| routes()).await?;

        // TODO: Add comprehensive integration tests for:
        // - Listing members with different permission levels
        // - Getting member details with proper authorization
        // - Removing members with various edge cases
        // - Error scenarios and permission validation
        // - Pagination behavior for large member lists

        Ok(())
    }

    #[tokio::test]
    async fn member_response_conversions() {
        // TODO: Add unit tests for response model conversions
        // - Test From<ProjectMember> implementations
        // - Verify all fields are properly mapped
        // - Check serialization behavior
    }

    #[tokio::test]
    async fn member_validation_logic() {
        // TODO: Add tests for business logic validation
        // - Self-removal prevention
        // - Permission level checks
        // - Member existence validation
    }
}
