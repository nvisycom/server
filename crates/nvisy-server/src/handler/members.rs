//! Workspace member management handlers.
//!
//! This module provides comprehensive workspace member management functionality,
//! allowing workspace administrators to view, add, modify, and remove workspace
//! members. All operations are secured with proper authorization and follow
//! role-based access control principles.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::query::{AccountRepository, WorkspaceMemberRepository};
use nvisy_postgres::types::{Username, WorkspaceRole};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::extract::{
    AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson, WorkspaceContext,
};
use crate::handler::request::{CursorPagination, ListMembers, MemberPathParams, UpdateMember};
use crate::handler::response::{ErrorResponse, Member, MembersPage, Page};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{ServiceState, WebhookEmitter};

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
        workspace_id = %workspace.id,
    )
)]
async fn list_members(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Query(query): Query<ListMembers>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<MembersPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace members");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewMembers)
        .await?;

    let page = conn
        .cursor_list_workspace_members_with_accounts(
            workspace.id,
            pagination.into(),
            query.to_filter(),
        )
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        member_count = page.items.len(),
        "Workspace members listed",
    );

    let response = Page::from_cursor_page(page, |(member, account)| {
        Member::from_model(member, account)
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_members_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List members")
        .description("Returns a paginated list of workspace members with their roles and status.")
        .response::<200, Json<MembersPage>>()
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
        workspace_id = %workspace.id,
        member = %path_params.username,
        member_id = tracing::field::Empty,
    )
)]
async fn get_member(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<MemberPathParams>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::debug!(target: TRACING_TARGET, "Retrieving workspace member details");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ViewMembers)
        .await?;

    let member_account_id = resolve_member_account_id(&mut conn, &path_params.username).await?;

    let Some((workspace_member, account)) = conn
        .find_workspace_member_with_account(workspace.id, member_account_id)
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

    Ok((
        StatusCode::OK,
        Json(Member::from_model(workspace_member, account)),
    ))
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
/// Requires `RemoveMembers` permission. Cannot remove an owner.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        member = %path_params.username,
        member_id = tracing::field::Empty,
    )
)]
async fn delete_member(
    State(pg_client): State<PgClient>,
    State(webhook_emitter): State<WebhookEmitter>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<MemberPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Removing workspace member");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::RemoveMembers)
        .await?;

    let member_account_id = resolve_member_account_id(&mut conn, &path_params.username).await?;

    // Prevent self-removal (use leave endpoint instead)
    if auth_state.account_id == member_account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot remove yourself. Use the leave workspace endpoint instead."));
    }

    let Some(member_to_remove) = conn
        .find_workspace_member(workspace.id, member_account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("workspace_member"));
    };

    // Owners cannot be removed, they can only leave
    if member_to_remove.member_role == WorkspaceRole::Owner {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot remove an owner")
            .with_context("Owners can only leave the workspace themselves"));
    }

    conn.remove_workspace_member(workspace.id, member_account_id)
        .await?;

    // Emit webhook event (fire-and-forget)
    let data = serde_json::json!({
        "removedUsername": path_params.username,
    });
    if let Err(err) = webhook_emitter
        .emit_member_deleted(
            workspace.id,
            member_account_id,
            Some(auth_state.account_id),
            Some(data),
        )
        .await
    {
        tracing::warn!(
            target: TRACING_TARGET,
            error = %err,
            "Failed to emit member:deleted webhook event"
        );
    }

    tracing::warn!(target: TRACING_TARGET, "Workspace member removed");

    Ok(StatusCode::OK)
}

fn delete_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Remove member")
        .description(
            "Permanently removes a member from the workspace. Cannot remove owners or yourself.",
        )
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates a workspace member's role.
///
/// Allows workspace owners to change a member's permission level.
/// Cannot update your own role. Cannot demote an owner.
/// Requires `ManageRoles` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        member = %path_params.username,
        member_id = tracing::field::Empty,
        new_role = ?request.role,
    )
)]
async fn update_member(
    State(pg_client): State<PgClient>,
    State(webhook_emitter): State<WebhookEmitter>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    Path(path_params): Path<MemberPathParams>,
    ValidateJson(request): ValidateJson<UpdateMember>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace member role");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, workspace.id, Permission::ManageRoles)
        .await?;

    let member_account_id = resolve_member_account_id(&mut conn, &path_params.username).await?;

    // Prevent self-role-update
    if auth_state.account_id == member_account_id {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot update your own role")
            .with_context("Ask another owner to update your role"));
    }

    let Some(current_member) = conn
        .find_workspace_member(workspace.id, member_account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("workspace_member"));
    };

    // Owners cannot be demoted, they can only leave
    if current_member.member_role == WorkspaceRole::Owner && request.role != WorkspaceRole::Owner {
        return Err(ErrorKind::BadRequest
            .with_message("Cannot demote an owner")
            .with_context("Owners can only leave the workspace themselves"));
    }

    let new_role = request.role;
    conn.update_workspace_member(workspace.id, member_account_id, request.into_model())
        .await?;

    let Some((updated_member, account)) = conn
        .find_workspace_member_with_account(workspace.id, member_account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound.with_resource("workspace_member"));
    };

    // Emit webhook event (fire-and-forget)
    let data = serde_json::json!({
        "username": path_params.username,
        "previousRole": current_member.member_role.to_string(),
        "newRole": new_role.to_string(),
    });
    if let Err(err) = webhook_emitter
        .emit_member_updated(
            workspace.id,
            member_account_id,
            Some(auth_state.account_id),
            Some(data),
        )
        .await
    {
        tracing::warn!(
            target: TRACING_TARGET,
            error = %err,
            "Failed to emit member:updated webhook event"
        );
    }

    tracing::info!(
        target: TRACING_TARGET,
        new_role = ?updated_member.member_role,
        "Member role updated",
    );

    Ok((
        StatusCode::OK,
        Json(Member::from_model(updated_member, account)),
    ))
}

fn update_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update member role")
        .description(
            "Updates a workspace member's role. Cannot update your own role or demote owners.",
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
/// The last owner cannot leave - they must transfer ownership first.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
    )
)]
async fn leave_workspace(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Member leaving workspace");

    let mut conn = pg_client.get_connection().await?;

    let Some(_member) = conn
        .find_workspace_member(workspace.id, auth_state.account_id)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("workspace_member")
            .with_message("You are not a member of this workspace"));
    };

    conn.remove_workspace_member(workspace.id, auth_state.account_id)
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

/// Resolves a member's public handle to its account id, recording the id on the
/// current tracing span. Returns `NotFound` when no such account exists.
async fn resolve_member_account_id(conn: &mut PgConn, username: &Username) -> Result<Uuid> {
    let account = conn
        .find_account_by_username(username)
        .await?
        .ok_or_else(|| Error::not_found("workspace_member"))?;
    tracing::Span::current().record("member_id", tracing::field::display(account.id));
    Ok(account.id)
}

/// Returns a [`Router`] with all workspace member related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceSlug}/members/",
            get_with(list_members, list_members_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/members/leave/",
            post_with(leave_workspace, leave_workspace_docs),
        )
        .api_route(
            "/workspaces/{workspaceSlug}/members/{username}/",
            get_with(get_member, get_member_docs)
                .patch_with(update_member, update_member_docs)
                .delete_with(delete_member, delete_member_docs),
        )
        .with_path_items(|item| item.tag("Members"))
}
