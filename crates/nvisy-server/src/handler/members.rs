//! Project member management handlers.
//!
//! This module provides comprehensive project member management functionality,
//! allowing project administrators to view, add, modify, and remove project
//! members. All operations are secured with proper authorization and follow
//! role-based access control principles.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::UpdateProjectMember;
use nvisy_postgres::query::{ProjectMemberRepository, ProjectRepository};
use nvisy_postgres::types::{ProjectRole, ProjectVisibility};
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Permission, ValidateJson};
use crate::handler::projects::ProjectPathParams;
use crate::handler::request::UpdateMemberRole;
use crate::handler::response::{Member, Members};
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for project member operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_members";

/// Combined path parameters for member-specific endpoints.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct MemberPathParams {
    /// Unique identifier of the project.
    pub project_id: Uuid,
    /// Unique identifier of the member account.
    pub account_id: Uuid,
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
            description = "Access denied: insufficient permissions",
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
            body = Members,
        ),
    ),
)]
async fn list_members(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Members>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Listing project members"
    );

    // Verify user has permission to view project members
    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::ViewMembers)
        .await?;

    // Fetch project to check if it's private
    let Some(project) = pg_client.find_project_by_id(path_params.project_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("project")
            .with_message("Project not found")
            .with_context(format!("Project ID: {}", path_params.project_id)));
    };

    // If project is private, return empty member list
    let is_private = project.visibility == ProjectVisibility::Private;
    let members: Members = if is_private {
        Vec::new()
    } else {
        // Retrieve project members with pagination
        let project_members = pg_client
            .list_project_members(path_params.project_id, pagination.into())
            .await?;

        project_members.into_iter().map(Member::from).collect()
    };

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_count = members.len(),
        "Project members listed successfully"
    );

    Ok((StatusCode::OK, Json(members)))
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
            body = Member,
        ),
    )
)]
async fn get_member(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        "Retrieving project member details"
    );

    // Verify user has permission to view project member details
    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::ViewMembers)
        .await?;

    // Check if project is private
    let Some(project) = pg_client.find_project_by_id(path_params.project_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("project")
            .with_message("Project not found")
            .with_context(format!("Project ID: {}", path_params.project_id)));
    };

    if project.visibility.is_private() {
        return Err(ErrorKind::Forbidden
            .with_resource("project")
            .with_message("Cannot view members of a private project")
            .with_context(format!("Project ID: {}", path_params.project_id)));
    }

    // Find the specific project member
    let Some(project_member) = pg_client
        .find_project_member(path_params.project_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("project member")
            .with_message("Project member not found")
            .with_context(format!(
                "Project ID: {}, Account ID: {}",
                path_params.project_id, path_params.account_id
            )));
    };

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        member_role = ?project_member.member_role,
        "Project member retrieved successfully"
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
        ),
    )
)]
async fn delete_member(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        "Removing project member"
    );

    // Verify user has permission to remove members
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::RemoveMembers,
        )
        .await?;

    // Prevent users from removing themselves (they should use leave endpoint instead)
    if auth_claims.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest.with_context(
            "Cannot remove yourself from the project. Use the leave project endpoint instead.",
        ));
    }

    // Get the member being removed
    let Some(member_to_remove) = pg_client
        .find_project_member(path_params.project_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("project member"));
    };

    // Check if removing the last owner
    if member_to_remove.member_role == nvisy_postgres::types::ProjectRole::Owner {
        // Count how many active owners exist
        let all_members = pg_client
            .list_project_members(
                path_params.project_id,
                nvisy_postgres::query::Pagination {
                    limit: 1000,
                    offset: 0,
                },
            )
            .await?;

        let owner_count = all_members
            .iter()
            .filter(|m| m.member_role == nvisy_postgres::types::ProjectRole::Owner && m.is_active)
            .count();

        if owner_count <= 1 {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot remove last owner")
                .with_context(
                    "Projects must have at least one owner. Transfer ownership or promote another member to owner before removing this member."
                ));
        }
    }

    // Remove the member from the project
    pg_client
        .remove_project_member(path_params.project_id, path_params.account_id)
        .await?;

    tracing::warn!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        "Project member removed successfully"
    );

    Ok(StatusCode::OK)
}

/// Updates a project member's role.
///
/// Allows project owners/admins to change a member's permission level.
/// Cannot update your own role. Cannot remove the last owner.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/projects/{projectId}/members/{accountId}/role", tag = "projects",
    params(MemberPathParams),
    request_body = UpdateMemberRole,
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
            body = Member,
        ),
    )
)]
async fn update_member(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<MemberPathParams>,
    ValidateJson(request): ValidateJson<UpdateMemberRole>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        member_id = path_params.account_id.to_string(),
        new_role = ?request.role,
        "Updating project member role"
    );

    // Verify user has permission to manage member roles
    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::ManageRoles)
        .await?;

    // Prevent users from updating their own role
    if auth_claims.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot update your own role")
            .with_context("Ask another project owner or admin to update your role"));
    }

    // Get the current member
    // Get current member information
    let Some(current_member) = pg_client
        .find_project_member(path_params.project_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("project member"));
    };

    // If demoting from owner, check we're not removing the last owner
    if current_member.member_role == ProjectRole::Owner && request.role != ProjectRole::Owner {
        let all_members = pg_client
            .list_project_members(
                path_params.project_id,
                nvisy_postgres::query::Pagination {
                    limit: 1000,
                    offset: 0,
                },
            )
            .await?;

        let owner_count = all_members
            .iter()
            .filter(|m| m.member_role == ProjectRole::Owner && m.is_active)
            .count();

        if owner_count <= 1 {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot demote last owner")
                .with_context(
                    "Projects must have at least one owner. Promote another member to owner first.",
                ));
        }
    }

    // Update the role
    let changes = UpdateProjectMember {
        member_role: Some(request.role),
        custom_permissions: None,
        show_order: None,
        is_favorite: None,
        is_hidden: None,
        notify_updates: None,
        notify_comments: None,
        notify_mentions: None,
        is_active: None,
        last_accessed_at: None,
        updated_by: None,
    };

    let updated_member = pg_client
        .update_project_member(path_params.project_id, path_params.account_id, changes)
        .await
        .map_err(|err| {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                member_id = %path_params.account_id,
                "Failed to update member role"
            );
            ErrorKind::InternalServerError.with_message("Failed to update member role")
        })?;

    tracing::info!(
        target: TRACING_TARGET,
        member_id = %updated_member.account_id,
        project_id = %updated_member.project_id,
        new_role = ?updated_member.member_role,
        "Member role updated successfully"
    );

    Ok((StatusCode::OK, Json(updated_member.into())))
}

/// Returns a [`Router`] with all project member related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new()
        .routes(routes!(list_members))
        .routes(routes!(get_member, update_member, delete_member))
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
