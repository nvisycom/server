//! Workspace member management handlers.
//!
//! Lists, reads, updates, and removes workspace members, and lets a member leave.
//! Each handler authorizes the caller, delegates the domain logic to
//! [`WorkspaceMemberService`], and maps the result to a response.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;

use crate::domain;
use crate::extract::{
    AuthState, Authorized, Json, Path, Query, SecurityContext, ValidateJson, WorkspaceContext,
    markers,
};
use crate::handler::request::{
    CursorPagination, ListWorkspaceMembers, UpdateWorkspaceMember, WorkspaceMemberPathParams,
};
use crate::handler::response::{Page, WorkspaceMember, WorkspaceMembersPage};
use crate::response::{ErrorResponse, Result};
use crate::service::ServiceState;
use crate::service::event::EventOrigin;

/// Tracing target for workspace member operations.
const TRACING_TARGET: &str = "nvisy_server::handler::members";

/// Lists all members of a workspace.
///
/// Returns a paginated list of workspace members with their roles and status.
/// Requires `ViewMembers` permission. Returns an empty list for private workspaces.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_members(
    State(members): State<domain::WorkspaceMemberService>,
    authz: Authorized<markers::ViewMembers>,
    Query(query): Query<ListWorkspaceMembers>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspaceMembersPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace members");

    let workspace = authz.workspace;
    let page = members
        .list(
            workspace.id,
            pagination.into_cursor(),
            query.to_sort(),
            query.to_filter(),
        )
        .await?;

    let response = Page::from_cursor_page(page, |(member, account)| {
        WorkspaceMember::from_model(&member, account)
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_members_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List members")
        .description("Returns a paginated list of workspace members with their roles and status.")
        .response::<200, Json<WorkspaceMembersPage>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn get_member(
    State(members): State<domain::WorkspaceMemberService>,
    authz: Authorized<markers::ViewMembers>,
    Path(path_params): Path<WorkspaceMemberPathParams>,
) -> Result<(StatusCode, Json<WorkspaceMember>)> {
    tracing::debug!(target: TRACING_TARGET, "Retrieving workspace member details");

    let workspace = authz.workspace;
    let (member, account) = members.find(workspace.id, path_params.account_id).await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceMember::from_model(&member, account)),
    ))
}

fn get_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get member")
        .description("Returns detailed information about a specific workspace member.")
        .response::<200, Json<WorkspaceMember>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn delete_member(
    State(members): State<domain::WorkspaceMemberService>,
    authz: Authorized<markers::RemoveMembers>,
    Path(path_params): Path<WorkspaceMemberPathParams>,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Removing workspace member");

    let workspace = authz.workspace;
    members
        .remove(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            path_params.account_id,
        )
        .await?;

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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        new_role = ?request.role,
    )
)]
async fn update_member(
    State(members): State<domain::WorkspaceMemberService>,
    authz: Authorized<markers::ManageRoles>,
    Path(path_params): Path<WorkspaceMemberPathParams>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<UpdateWorkspaceMember>,
) -> Result<(StatusCode, Json<WorkspaceMember>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating workspace member role");

    let workspace = authz.workspace;
    let (member, account) = members
        .update(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            path_params.account_id,
            request.into_model(),
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceMember::from_model(&member, account)),
    ))
}

fn update_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update member role")
        .description(
            "Updates a workspace member's role. Cannot update your own role or demote owners.",
        )
        .response::<200, Json<WorkspaceMember>>()
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
    State(members): State<domain::WorkspaceMemberService>,
    auth_state: AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    security: SecurityContext,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Workspace member leaving workspace");

    members
        .leave(EventOrigin {
            workspace_id: workspace.id,
            account_id: auth_state.account_id,
            security: &security,
        })
        .await?;

    Ok(StatusCode::OK)
}

fn leave_workspace_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Leave workspace")
        .description("Allows a member to voluntarily leave a workspace.")
        .response::<200, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all workspace member related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/members",
            get_with(list_members, list_members_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/members/leave",
            post_with(leave_workspace, leave_workspace_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/members/{accountId}",
            get_with(get_member, get_member_docs)
                .patch_with(update_member, update_member_docs)
                .delete_with(delete_member, delete_member_docs),
        )
        .with_path_items(|item| item.tag("Members"))
}
