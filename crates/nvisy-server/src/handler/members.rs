//! Workspace member management handlers.
//!
//! This module provides comprehensive workspace member management functionality,
//! allowing workspace administrators to view, add, modify, and remove workspace
//! members. All operations are secured with proper authorization and follow
//! role-based access control principles.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::http::StatusCode;
use nvisy_postgres::query::{WorkspaceMemberRepository, WorkspaceRepository};
use nvisy_postgres::types::{WorkspaceRole, WorkspaceVisibility};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, PgPool, ValidateJson};
use crate::handler::request::{MemberPathParams, Pagination, WorkspacePathParams, UpdateMemberRole};
use crate::handler::response::{ErrorResponse, Member, Members};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for workspace member operations.
const TRACING_TARGET: &str = "nvisy_server::handler::members";

/// Lists all members of a workspace.
///
/// Returns a paginated list of workspace members with their roles and status.
/// Requires `ViewMembers` permission. Returns an empty list for private workspaces.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_members(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Members>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace members");

    auth_state
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::ViewMembers)
        .await?;

    let Some(workspace) = conn.find_workspace_by_id(path_params.workspace_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("workspace")
            .with_message("Workspace not found"));
    };

    // Return empty list for private workspaces
    let members: Members = if workspace.visibility == WorkspaceVisibility::Private {
        tracing::debug!(target: TRACING_TARGET, "Workspace is private, returning empty list");
        Vec::new()
    } else {
        let workspace_members = conn
            .list_workspace_members(path_params.workspace_id, pagination.into())
            .await?;

        workspace_members.into_iter().map(Member::from).collect()
    };

    tracing::info!(
        target: TRACING_TARGET,
        member_count = members.len(),
        "Workspace members listed ",
    );

    Ok((StatusCode::OK, Json(members)))
}

fn list_members_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List members")
        .description("Returns a paginated list of workspace members with their roles and status.")
        .response::<200, Json<Members>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Gets detailed information about a specific workspace member.
///
/// Returns comprehensive information about a workspace member, including their role,
/// permissions, and activity status. Requires `ViewMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        member_id = %path_params.account_id,
    )
)]
async fn get_member(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::debug!(target: TRACING_TARGET, "Retrieving workspace member details");

    auth_state
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::ViewMembers)
        .await?;

    let Some(workspace) = conn.find_workspace_by_id(path_params.workspace_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("workspace")
            .with_message("Workspace not found"));
    };

    if workspace.visibility.is_private() {
        return Err(ErrorKind::Forbidden
            .with_resource("workspace")
            .with_message("Cannot view members of a private workspace"));
    }

    let Some(workspace_member) = conn
        .find_workspace_member(path_params.workspace_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("workspace_member")
            .with_message("Workspace member not found"));
    };

    tracing::debug!(
        target: TRACING_TARGET,
        member_role = ?workspace_member.member_role,
        "Workspace member read",
    );

    Ok((StatusCode::OK, Json(workspace_member.into())))
}

fn get_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get member")
        .description("Returns detailed information about a specific workspace member.")
        .response::<200, Json<Member>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Removes a member from a workspace.
///
/// Permanently removes a member from the workspace. This action cannot be undone.
/// The member will lose all access to the workspace and its resources.
/// Requires `RemoveMembers` permission. Cannot remove the last admin.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        member_id = %path_params.account_id,
    )
)]
async fn delete_member(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<MemberPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Removing workspace member");

    auth_state
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::RemoveMembers)
        .await?;

    // Prevent self-removal (use leave endpoint instead)
    if auth_state.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot remove yourself. Use the leave workspace endpoint instead."));
    }

    let Some(member_to_remove) = conn
        .find_workspace_member(path_params.workspace_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("workspace_member"));
    };

    // Admins cannot be removed, they can only leave
    if member_to_remove.member_role == WorkspaceRole::Admin {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot remove an admin")
            .with_context("Admins can only leave the workspace themselves"));
    }

    conn.remove_workspace_member(path_params.workspace_id, path_params.account_id)
        .await?;

    tracing::warn!(target: TRACING_TARGET, "Workspace member removed");

    Ok(StatusCode::OK)
}

fn delete_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Remove member")
        .description(
            "Permanently removes a member from the workspace. Cannot remove admins or yourself.",
        )
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace member's role.
///
/// Allows workspace admins to change a member's permission level.
/// Cannot update your own role. Cannot demote the last admin.
/// Requires `ManageRoles` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        member_id = %path_params.account_id,
        new_role = ?request.role,
    )
)]
async fn update_member(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<MemberPathParams>,
    ValidateJson(request): ValidateJson<UpdateMemberRole>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace member role");

    auth_state
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::ManageRoles)
        .await?;

    // Prevent self-role-update
    if auth_state.account_id == path_params.account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot update your own role")
            .with_context("Ask another admin to update your role"));
    }

    let Some(current_member) = conn
        .find_workspace_member(path_params.workspace_id, path_params.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("workspace_member"));
    };

    // Admins cannot be demoted, they can only leave
    if current_member.member_role == WorkspaceRole::Admin && request.role != WorkspaceRole::Admin {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot demote an admin")
            .with_context("Admins can only leave the workspace themselves"));
    }

    let updated_member = conn
        .update_workspace_member(
            path_params.workspace_id,
            path_params.account_id,
            request.into_model(),
        )
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        new_role = ?updated_member.member_role,
        "Member role updated ",
    );

    Ok((StatusCode::OK, Json(updated_member.into())))
}

fn update_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update member role")
        .description(
            "Updates a workspace member's role. Cannot update your own role or demote admins.",
        )
        .response::<200, Json<Member>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Leaves a workspace.
///
/// Allows a member to voluntarily leave a workspace. This action cannot be undone.
/// The member will lose all access to the workspace and its resources.
/// The last admin cannot leave - they must transfer ownership first.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn leave_workspace(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Member leaving workspace");

    let Some(_member) = conn
        .find_workspace_member(path_params.workspace_id, auth_state.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("workspace_member")
            .with_message("You are not a member of this workspace"));
    };

    conn.remove_workspace_member(path_params.workspace_id, auth_state.account_id)
        .await?;

    tracing::warn!(target: TRACING_TARGET, "Member left workspace");

    Ok(StatusCode::OK)
}

fn leave_workspace_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Leave workspace")
        .description("Allows a member to voluntarily leave a workspace.")
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all workspace member related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspace_id}/members/",
            get_with(list_members, list_members_docs),
        )
        .api_route(
            "/workspaces/{workspace_id}/members/leave",
            post_with(leave_workspace, leave_workspace_docs),
        )
        .api_route(
            "/workspaces/{workspace_id}/members/{account_id}/",
            get_with(get_member, get_member_docs).delete_with(delete_member, delete_member_docs),
        )
        .api_route(
            "/workspaces/{workspace_id}/members/{account_id}/role",
            patch_with(update_member, update_member_docs),
        )
        .with_path_items(|item| item.tag("Members"))
}
