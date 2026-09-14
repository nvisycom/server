//! Workspace invitation management handlers.
//!
//! Sends, lists, cancels, and replies to workspace invitations, and mints and
//! consumes shareable invite codes. Each handler authorizes the caller, delegates
//! the domain logic to [`WorkspaceInviteService`], and maps the result to a
//! response. The reply-by-code and preview-by-code endpoints are public.

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
    CreateWorkspaceInvite, CursorPagination, GenerateWorkspaceInviteCode, InviteCodePathParams,
    ListWorkspaceInvites, ReplyWorkspaceInvite, WorkspaceInvitePathParams,
};
use crate::handler::response::{
    InvitePreview, WorkspaceInvite, WorkspaceInviteCode, WorkspaceInviteSent, WorkspaceInvitesPage,
    WorkspaceMember,
};
use crate::response::{ErrorResponse, Result};
use crate::service::ServiceState;
use crate::service::event::EventOrigin;

/// Tracing target for workspace invite operations.
const TRACING_TARGET: &str = "nvisy_server::handler::invites";

/// Creates a new workspace invitation.
///
/// Invites an existing platform user to the workspace. This server sends no
/// email; if the address does not belong to a known account, the request still
/// succeeds but nothing is created, so the response cannot reveal whether an
/// account exists. Requires `InviteMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        invited_role = ?request.invited_role,
    )
)]
async fn send_invite(
    State(invites): State<domain::WorkspaceInviteService>,
    authz: Authorized<markers::InviteMembers>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateWorkspaceInvite>,
) -> Result<(StatusCode, Json<WorkspaceInviteSent>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating workspace invitation");

    let workspace = authz.workspace;
    let input = request.into();
    let outcome = invites
        .create(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            &input,
        )
        .await?;

    match outcome {
        domain::output::InviteOutcome::Created(_) => {
            tracing::info!(target: TRACING_TARGET, "Workspace invitation created");
        }
        domain::output::InviteOutcome::UnknownEmail => {
            tracing::debug!(target: TRACING_TARGET, "Invite email has no account; no-op");
        }
    }

    Ok((StatusCode::OK, Json(WorkspaceInviteSent::new())))
}

fn send_invite_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Send invitation")
        .description(
            "Invites an existing platform user to the workspace. No email is sent by this \
             server. The response is identical whether or not the address belongs to a known \
             account, so it cannot be used to determine whether an account exists.",
        )
        .response::<200, Json<WorkspaceInviteSent>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
    )
)]
async fn list_invites(
    State(invites): State<domain::WorkspaceInviteService>,
    authz: Authorized<markers::ViewMembers>,
    Query(query): Query<ListWorkspaceInvites>,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<WorkspaceInvitesPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing workspace invitations");

    let workspace = authz.workspace;
    let page = invites
        .list(
            workspace.id,
            pagination.into_cursor(),
            query.to_sort(),
            query.to_filter(),
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(WorkspaceInvitesPage::from_cursor_page(page, |invite| {
            WorkspaceInvite::from_model(invite, workspace.id, workspace.handle.clone())
        })),
    ))
}

fn list_invites_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List invitations")
        .description("Returns a paginated list of workspace invitations with their current status.")
        .response::<200, Json<WorkspaceInvitesPage>>()
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
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        invite_id = %path_params.invite_id,
    )
)]
async fn cancel_invite(
    State(invites): State<domain::WorkspaceInviteService>,
    authz: Authorized<markers::InviteMembers>,
    security: SecurityContext,
    Path(path_params): Path<WorkspaceInvitePathParams>,
) -> Result<StatusCode> {
    tracing::info!(target: TRACING_TARGET, "Cancelling workspace invitation");

    let workspace = authz.workspace;
    invites
        .cancel(
            EventOrigin {
                workspace_id: workspace.id,
                account_id: authz.account_id,
                security: &security,
            },
            path_params.invite_id,
        )
        .await?;

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
/// Allows the invitee to accept or decline a workspace invitation. If accepted,
/// the user becomes a member of the workspace with the specified role. The
/// invitation must be valid and not expired.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        workspace_id = %workspace.id,
        invite_id = %path_params.invite_id,
        accept = request.accept_invite,
    )
)]
async fn reply_to_invite(
    State(invites): State<domain::WorkspaceInviteService>,
    auth_state: AuthState,
    WorkspaceContext(workspace): WorkspaceContext,
    security: SecurityContext,
    Path(path_params): Path<WorkspaceInvitePathParams>,
    Json(request): Json<ReplyWorkspaceInvite>,
) -> Result<(StatusCode, Json<Option<WorkspaceMember>>)> {
    tracing::info!(target: TRACING_TARGET, "Responding to workspace invitation");

    let origin = EventOrigin {
        workspace_id: workspace.id,
        account_id: auth_state.account_id,
        security: &security,
    };

    if request.accept_invite {
        let accepted = invites.accept(origin, path_params.invite_id).await?;
        let member = WorkspaceMember::from_model(&accepted.member, accepted.account);
        Ok((StatusCode::CREATED, Json(Some(member))))
    } else {
        invites.decline(origin, path_params.invite_id).await?;
        Ok((StatusCode::OK, Json(None)))
    }
}

fn reply_to_invite_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reply to invitation")
        .description(
            "Accepts or declines a workspace invitation. On accept the user becomes a \
             member and the new membership is returned; on decline no membership is created.",
        )
        .response::<200, Json<Option<WorkspaceMember>>>()
        .response::<201, Json<WorkspaceMember>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Generates a shareable invite code for a workspace.
///
/// Creates an invite code that can be shared with anyone to join the workspace.
/// The code is single-use: it is consumed by the first account that accepts it,
/// and expires if unused. Requires `InviteMembers` permission.
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %authz.account_id,
        workspace_id = %authz.workspace.id,
        invited_role = ?request.invited_role,
    )
)]
async fn generate_invite_code(
    State(invites): State<domain::WorkspaceInviteService>,
    authz: Authorized<markers::InviteMembers>,
    ValidateJson(request): ValidateJson<GenerateWorkspaceInviteCode>,
) -> Result<(StatusCode, Json<WorkspaceInviteCode>)> {
    tracing::info!(target: TRACING_TARGET, "Generating invite code");

    let workspace = authz.workspace;
    let invite = invites
        .generate_code(workspace.id, authz.account_id, request.into())
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(WorkspaceInviteCode::from_invite(
            &invite,
            workspace.id,
            workspace.handle,
        )),
    ))
}

fn generate_invite_code_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Generate invite code")
        .description(
            "Creates a shareable, single-use invite code that lets one person join the \
             workspace. The code is consumed on first acceptance and expires if unused.",
        )
        .response::<201, Json<WorkspaceInviteCode>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
}

/// Previews a workspace invitation before joining.
///
/// Returns basic workspace information for an invite code, allowing users to see
/// what workspace they're about to join before accepting. This endpoint does not
/// require authentication.
#[tracing::instrument(skip_all)]
async fn preview_invite_code(
    State(invites): State<domain::WorkspaceInviteService>,
    Path(path_params): Path<InviteCodePathParams>,
) -> Result<(StatusCode, Json<InvitePreview>)> {
    tracing::debug!(target: TRACING_TARGET, "Previewing invite code");

    let preview = invites.preview_code(&path_params.invite_code).await?;

    Ok((
        StatusCode::OK,
        Json(InvitePreview::from_models(
            preview.workspace,
            &preview.invite,
        )),
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
/// Allows a user to accept or decline a workspace invite code. If accepted (the
/// default), the user is added as a member with the role specified when the code
/// was generated. If declined, no action is taken.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn reply_to_invite_code(
    State(invites): State<domain::WorkspaceInviteService>,
    auth_state: AuthState,
    security: SecurityContext,
    Path(path_params): Path<InviteCodePathParams>,
    request: Option<Json<ReplyWorkspaceInvite>>,
) -> Result<(StatusCode, Json<Option<WorkspaceMember>>)> {
    let accept = request.is_none_or(|Json(r)| r.accept_invite);

    tracing::info!(target: TRACING_TARGET, accept, "Responding to invite code");

    if accept {
        let accepted = invites
            .accept_code(auth_state.account_id, &security, &path_params.invite_code)
            .await?;
        let member = WorkspaceMember::from_model(&accepted.member, accepted.account);
        Ok((StatusCode::CREATED, Json(Some(member))))
    } else {
        invites
            .decline_code(auth_state.account_id, &security, &path_params.invite_code)
            .await?;
        Ok((StatusCode::OK, Json(None)))
    }
}

fn reply_to_invite_code_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Reply to invite code")
        .description("Accepts or declines a workspace invite code. If accepted (the default when no body is provided), the user becomes a member with the role specified in the code. If declined, no action is taken.")
        .response::<200, Json<Option<WorkspaceMember>>>()
        .response::<201, Json<WorkspaceMember>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all workspace invite related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::{delete_with, get_with, post_with};

    ApiRouter::new()
        .api_route(
            "/workspaces/{workspaceId}/invites/",
            post_with(send_invite, send_invite_docs).get_with(list_invites, list_invites_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/invites/code/",
            post_with(generate_invite_code, generate_invite_code_docs),
        )
        .api_route(
            "/workspaces/{workspaceId}/invites/{inviteId}/",
            delete_with(cancel_invite, cancel_invite_docs)
                .post_with(reply_to_invite, reply_to_invite_docs),
        )
        .api_route(
            "/invites/code/{inviteCode}/",
            get_with(preview_invite_code, preview_invite_code_docs)
                .post_with(reply_to_invite_code, reply_to_invite_code_docs),
        )
        .with_path_items(|item| item.tag("Invites"))
}
