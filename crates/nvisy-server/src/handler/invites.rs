//! Workspace invitation management handlers.
//!
//! This module provides comprehensive workspace invitation functionality, allowing
//! workspace administrators to invite users to join workspaces with specific roles.
//! All operations are secured with proper authorization and include invitation
//! lifecycle management.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::NewWorkspaceMember;
use nvisy_postgres::query::{
    WorkspaceInviteRepository, WorkspaceMemberRepository, WorkspaceRepository,
};
use nvisy_postgres::{PgClient, PgError};

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, Query, ValidateJson};
use crate::handler::request::{
    CreateInvite, CursorPagination, GenerateInviteCode, InviteCodePathParams, InvitePathParams,
    ListInvites, ReplyInvite, WorkspacePathParams,
};
use crate::handler::response::{
    ErrorResponse, Invite, InviteCode, InvitePreview, InvitesPage, Member,
};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for workspace invite operations.
const TRACING_TARGET: &str = "nvisy_server::handler::invites";

/// Creates a new workspace invitation.
///
/// Sends an invitation to join a workspace to the specified email address.
/// The invitee will receive an email with instructions to accept or decline.
/// Requires `InviteMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        invited_role = ?request.invited_role,
    )
)]
async fn send_invite(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<CreateInvite>,
) -> Result<(StatusCode, Json<Invite>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace invitation");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::InviteMembers,
        )
        .await?;

    // Check if invitee is already a member (by email)
    if conn
        .find_workspace_member_by_email(path_params.workspace_id, &request.invitee_email)
        .await?
        .is_some()
    {
        return Err(ErrorKind::Conflict
            .with_message("User is already a member of this workspace")
            .with_resource("workspace_member"));
    }

    // Check for existing pending invite for this email
    if conn
        .find_pending_workspace_invite_by_email(path_params.workspace_id, &request.invitee_email)
        .await?
        .is_some()
    {
        return Err(ErrorKind::Conflict
            .with_message("A pending invitation already exists for this email")
            .with_resource("workspace_invite"));
    }

    let workspace_invite = conn
        .create_workspace_invite(
            request.into_model(path_params.workspace_id, auth_state.account_id),
        )
        .await?;
    let response = Invite::from_model(workspace_invite);

    tracing::info!(
        target: TRACING_TARGET,
        invite_id = %response.invite_id,
        "Workspace invitation created",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn send_invite_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Send invitation")
        .description("Sends an invitation to join a workspace to the specified email address.")
        .response::<201, Json<Invite>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Lists all invitations for a workspace.
///
/// Returns a paginated list of workspace invitations with their current status.
/// Requires `ViewMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
    )
)]
async fn list_invites(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    Query(query): Query<ListInvites>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<InvitesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace invitations");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(&mut conn, path_params.workspace_id, Permission::ViewMembers)
        .await?;

    let page = conn
        .cursor_list_workspace_invites(
            path_params.workspace_id,
            pagination.into(),
            query.to_filter(),
        )
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        invite_count = page.items.len(),
        "Workspace invitations listed",
    );

    Ok((
        StatusCode::OK,
        Json(InvitesPage::from_cursor_page(page, Invite::from_model)),
    ))
}

fn list_invites_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List invitations")
        .description("Returns a paginated list of workspace invitations with their current status.")
        .response::<200, Json<InvitesPage>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Cancels a workspace invitation.
///
/// Permanently cancels a pending invitation. The invitee will no longer be able
/// to accept this invitation. Requires `InviteMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        invite_id = %path_params.invite_id,
    )
)]
async fn cancel_invite(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<InvitePathParams>,
) -> Result<StatusCode> {
    tracing::info!(target: TRACING_TARGET, "Cancelling workspace invitation");

    let mut conn = pg_client.get_connection().await?;

    // Fetch the invite first to get workspace context for authorization
    let invite = find_invite(&mut conn, path_params.invite_id).await?;

    auth_state
        .authorize_workspace(&mut conn, invite.workspace_id, Permission::InviteMembers)
        .await?;

    conn.cancel_workspace_invite(path_params.invite_id, auth_state.account_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Workspace invitation cancelled");

    Ok(StatusCode::OK)
}

fn cancel_invite_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Cancel invitation")
        .description("Permanently cancels a pending invitation. The invitee will no longer be able to accept it.")
        .response::<200, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Responds to a workspace invitation.
///
/// Allows the invitee to accept or decline a workspace invitation.
/// If accepted, the user becomes a member of the workspace with the specified role.
/// The invitation must be valid and not expired.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        invite_id = %path_params.invite_id,
        accept = request.accept_invite,
    )
)]
async fn reply_to_invite(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<InvitePathParams>,
    Json(request): Json<ReplyInvite>,
) -> Result<(StatusCode, Json<Invite>)> {
    tracing::info!(target: TRACING_TARGET, "Responding to workspace invitation");

    let mut conn = pg_client.get_connection().await?;

    let invite = find_invite(&mut conn, path_params.invite_id).await?;

    // Verify invitation is still valid
    if !invite.can_be_used() {
        return Err(ErrorKind::BadRequest
            .with_message("This invitation has expired or is no longer valid")
            .with_resource("workspace_invite"));
    }

    let workspace_invite = if request.accept_invite {
        // Check if user is already a member
        if conn
            .find_workspace_member(invite.workspace_id, auth_state.account_id)
            .await?
            .is_some()
        {
            return Err(ErrorKind::Conflict
                .with_message("You are already a member of this workspace")
                .with_resource("workspace_member"));
        }

        let invite_id = invite.id;
        let workspace_id = invite.workspace_id;
        let invited_role = invite.invited_role;
        let account_id = auth_state.account_id;

        let accepted = conn
            .transaction(|conn| {
                Box::pin(async move {
                    let accepted = conn.accept_workspace_invite(invite_id, account_id).await?;

                    let new_member =
                        NewWorkspaceMember::new(workspace_id, account_id, invited_role);
                    conn.add_workspace_member(new_member).await?;

                    Ok::<_, PgError>(accepted)
                })
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Invitation accepted");
        accepted
    } else {
        let declined = conn
            .reject_workspace_invite(path_params.invite_id, auth_state.account_id)
            .await?;

        tracing::info!(target: TRACING_TARGET, "Invitation declined");
        declined
    };

    Ok((StatusCode::OK, Json(Invite::from_model(workspace_invite))))
}

fn reply_to_invite_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reply to invitation")
        .description("Allows the invitee to accept or decline a workspace invitation.")
        .response::<200, Json<Invite>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Generates a shareable invite code for a workspace.
///
/// Creates an invite code that can be shared with anyone to join the workspace.
/// The code can be used multiple times until it expires.
/// Requires `InviteMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %path_params.workspace_id,
        invited_role = ?request.invited_role,
    )
)]
async fn generate_invite_code(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<WorkspacePathParams>,
    ValidateJson(request): ValidateJson<GenerateInviteCode>,
) -> Result<(StatusCode, Json<InviteCode>)> {
    tracing::info!(target: TRACING_TARGET, "Generating invite code");

    let mut conn = pg_client.get_connection().await?;

    auth_state
        .authorize_workspace(
            &mut conn,
            path_params.workspace_id,
            Permission::InviteMembers,
        )
        .await?;

    let workspace_invite = conn
        .create_workspace_invite(
            request.into_model(path_params.workspace_id, auth_state.account_id),
        )
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        invite_id = %workspace_invite.id,
        "Invite code generated ",
    );

    Ok((
        StatusCode::CREATED,
        Json(InviteCode::from_invite(&workspace_invite)),
    ))
}

fn generate_invite_code_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Generate invite code")
        .description(
            "Creates a shareable invite code that can be used by anyone to join the workspace.",
        )
        .response::<201, Json<InviteCode>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Previews a workspace invitation before joining.
///
/// Returns basic workspace information for an invite code, allowing users
/// to see what workspace they're about to join before accepting.
/// This endpoint does not require authentication.
#[tracing::instrument(skip_all)]
async fn preview_invite_code(
    State(pg_client): State<PgClient>,
    Path(path_params): Path<InviteCodePathParams>,
) -> Result<(StatusCode, Json<InvitePreview>)> {
    tracing::debug!(target: TRACING_TARGET, "Previewing invite code");

    let mut conn = pg_client.get_connection().await?;

    let Some(invite) = conn
        .find_workspace_invite_by_token(&path_params.invite_code)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("invite_code")
            .with_message("Invalid invite code"));
    };

    if !invite.can_be_used() {
        return Err(ErrorKind::BadRequest
            .with_message("This invite code has expired or is no longer valid")
            .with_resource("invite_code"));
    }

    let Some(workspace) = conn.find_workspace_by_id(invite.workspace_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("workspace")
            .with_message("Workspace not found"));
    };

    tracing::debug!(
        target: TRACING_TARGET,
        workspace_id = %workspace.id,
        "Invite preview retrieved"
    );

    Ok((
        StatusCode::OK,
        Json(InvitePreview::from_models(workspace, invite)),
    ))
}

fn preview_invite_code_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Preview invite")
        .description("Returns workspace information for an invite code, allowing users to preview the workspace before joining. Does not require authentication.")
        .response::<200, Json<InvitePreview>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Responds to a workspace invite code.
///
/// Allows a user to accept or decline a workspace invite code.
/// If accepted (the default), the user will be added as a member with the role
/// specified when the invite code was generated. If declined, no action is taken.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn reply_to_invite_code(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Path(path_params): Path<InviteCodePathParams>,
    Json(request): Json<Option<ReplyInvite>>,
) -> Result<(StatusCode, Json<Option<Member>>)> {
    let accept = request.map(|r| r.accept_invite).unwrap_or(true);

    tracing::info!(target: TRACING_TARGET, accept, "Responding to invite code");

    let mut conn = pg_client.get_connection().await?;

    let Some(invite) = conn
        .find_workspace_invite_by_token(&path_params.invite_code)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("invite_code")
            .with_message("Invalid invite code"));
    };

    if !invite.can_be_used() {
        return Err(ErrorKind::BadRequest
            .with_message("This invite code has expired or is no longer valid")
            .with_resource("invite_code"));
    }

    if accept {
        // Check if user is already a member
        if conn
            .find_workspace_member(invite.workspace_id, auth_state.account_id)
            .await?
            .is_some()
        {
            return Err(ErrorKind::Conflict
                .with_message("You are already a member of this workspace")
                .with_resource("workspace_member"));
        }

        let invite_id = invite.id;
        let workspace_id = invite.workspace_id;
        let invited_role = invite.invited_role;
        let account_id = auth_state.account_id;

        let (workspace_member, account) = conn
            .transaction(|conn| {
                Box::pin(async move {
                    conn.accept_workspace_invite(invite_id, account_id).await?;

                    let new_member =
                        NewWorkspaceMember::new(workspace_id, account_id, invited_role);
                    conn.add_workspace_member(new_member).await?;

                    let result = conn
                        .find_workspace_member_with_account(workspace_id, account_id)
                        .await?
                        .ok_or_else(|| {
                            PgError::Unexpected("Member not found after insert".into())
                        })?;

                    Ok::<_, PgError>(result)
                })
            })
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            workspace_id = %workspace_id,
            role = ?invited_role,
            "User joined workspace via invite code",
        );

        Ok((
            StatusCode::CREATED,
            Json(Some(Member::from_model(workspace_member, account))),
        ))
    } else {
        let workspace_id = invite.workspace_id;

        conn.reject_workspace_invite(invite.id, auth_state.account_id)
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            workspace_id = %workspace_id,
            "User declined invite code",
        );

        Ok((StatusCode::OK, Json(None)))
    }
}

fn reply_to_invite_code_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reply to invite code")
        .description("Accepts or declines a workspace invite code. If accepted (the default when no body is provided), the user becomes a member with the role specified in the code. If declined, no action is taken.")
        .response::<200, Json<Option<Member>>>()
        .response::<201, Json<Member>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Finds an invite by ID or returns NotFound error.
async fn find_invite(
    conn: &mut nvisy_postgres::PgConn,
    invite_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::WorkspaceInvite> {
    conn.find_workspace_invite_by_id(invite_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Invitation not found")
                .with_resource("workspace_invite")
        })
}

/// Returns a [`Router`] with all workspace invite related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        // Workspace-scoped routes (require workspace context)
        .api_route(
            "/workspaces/{workspaceId}/invites/",
            post_with(send_invite, send_invite_docs).get_with(list_invites, list_invites_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/invites/code/",
            post_with(generate_invite_code, generate_invite_code_docs),
        )
        // Invite-specific routes (invite ID is globally unique)
        .api_route(
            "/invites/{inviteId}/",
            delete_with(cancel_invite, cancel_invite_docs),
        )
        .api_route(
            "/invites/{inviteId}/",
            post_with(reply_to_invite, reply_to_invite_docs),
        )
        .api_route(
            "/invites/code/{inviteCode}/",
            get_with(preview_invite_code, preview_invite_code_docs),
        )
        .api_route(
            "/invites/code/{inviteCode}/",
            post_with(reply_to_invite_code, reply_to_invite_code_docs),
        )
        .with_path_items(|item| item.tag("Invites"))
}
