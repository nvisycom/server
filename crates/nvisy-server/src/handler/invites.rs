//! Project invitation management handlers.
//!
//! This module provides comprehensive project invitation functionality, allowing
//! project administrators to invite users to join projects with specific roles.
//! All operations are secured with proper authorization and include invitation
//! lifecycle management.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::NewProjectInvite;
use nvisy_postgres::query::{ProjectInviteRepository, ProjectMemberRepository};
use nvisy_postgres::types::InviteStatus;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::request::{
    CreateInvite, GenerateInviteCode, InviteCodePathParams, InvitePathParams, Pagination,
    ProjectPathParams, ReplyInvite,
};
use crate::handler::response::{Invite, InviteCode, Invites, Member};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for project invite operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_invites";

/// Creates a new project invitation.
///
/// Sends an invitation to join a project to the specified email address.
/// The invitee will receive an email with instructions to accept or decline.
/// Requires administrator permissions to send invitations.
#[tracing::instrument(skip_all)]
async fn send_invite(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateInvite>,
) -> Result<(StatusCode, Json<Invite>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invitee_email = %request.invitee_email,
        invited_role = ?request.invited_role,
        "Creating project invitation"
    );

    // Verify user has permission to send invitations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::InviteMembers,
        )
        .await?;

    // Check if user is already a member
    if let Some(existing_member) = pg_client
        .find_project_member(path_params.project_id, auth_claims.account_id)
        .await?
        && existing_member.is_active
    {
        return Err(ErrorKind::Conflict
            .with_message("User is already a member of this project")
            .with_resource("project_member")
            .with_context(format!(
                "Project ID: {}, Account ID: {}",
                path_params.project_id, auth_claims.account_id
            )));
    }

    // Check for existing pending invites to the same email
    let normalized_email = request.invitee_email.to_lowercase();
    let all_invites = pg_client
        .list_user_invites(
            None,
            nvisy_postgres::query::Pagination {
                limit: 100,
                offset: 0,
            },
        )
        .await?;

    // Filter by project_id since list_user_invites doesn't filter by project
    let existing_invites: Vec<_> = all_invites
        .into_iter()
        .filter(|invite| invite.project_id == path_params.project_id)
        .collect();

    // Check if there's already a pending invite
    if let Some(pending_invite) = existing_invites
        .iter()
        .find(|invite| invite.invite_status == InviteStatus::Pending)
    {
        return Err(ErrorKind::Conflict
            .with_message("Invitation already sent")
            .with_context(format!(
                "A pending invitation to {} already exists for this project (expires at {})",
                normalized_email,
                pending_invite.expires_at.to_jiff()
            )));
    }

    // Generate expiration time from InviteExpiration enum
    let expires_at = request.expires.to_expiry_timestamp();

    let new_invite = NewProjectInvite {
        project_id: path_params.project_id,
        invitee_id: None, // Will be set when user accepts if they have an account
        invited_role: Some(request.invited_role),
        expires_at: expires_at.map(Into::into),
        created_by: auth_claims.account_id,
        updated_by: auth_claims.account_id,
        ..Default::default()
    };

    let project_invite = pg_client.create_project_invite(new_invite).await?;

    let response = Invite::from(project_invite);

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = response.invite_id.to_string(),
        "Project invitation created successfully"
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Lists all invitations for a project.
///
/// Returns a paginated list of project invitations with their current status.
/// Optionally filter by invitation status. Requires administrator permissions.
#[tracing::instrument(skip_all)]
async fn list_invites(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<Invites>)> {
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "Listing project invitations"
    );

    // Verify user has permission to view project invitations
    auth_claims
        .authorize_project(&pg_client, path_params.project_id, Permission::ViewMembers)
        .await?;

    // Retrieve project invitations with pagination
    let project_invites = pg_client
        .list_project_invites(path_params.project_id, pagination.into())
        .await?;

    let invites: Invites = project_invites.into_iter().map(Invite::from).collect();

    tracing::debug!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_count = invites.len(),
        "Project invitations listed successfully"
    );

    Ok((StatusCode::OK, Json(invites)))
}

/// Cancels a project invitation.
///
/// Permanently cancels a pending invitation. The invitee will no longer be able
/// to accept this invitation. Requires administrator permissions.
#[tracing::instrument(skip_all)]
async fn cancel_invite(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<InvitePathParams>,
) -> Result<StatusCode> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = path_params.invite_id.to_string(),
        "Cancelling project invitation"
    );

    // Verify user has permission to manage project invitations
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::InviteMembers,
        )
        .await?;

    // Cancel the invitation
    pg_client
        .cancel_invite(path_params.invite_id, auth_claims.account_id)
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = path_params.invite_id.to_string(),
        "Project invitation cancelled successfully"
    );

    Ok(StatusCode::OK)
}

/// Responds to a project invitation.
///
/// Allows the invitee to accept or decline a project invitation.
/// If accepted, the user becomes a member of the project with the specified role.
/// The invitation must be valid and not expired.
#[tracing::instrument(skip_all)]
async fn reply_to_invite(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<InvitePathParams>,
    Json(request): Json<ReplyInvite>,
) -> Result<(StatusCode, Json<Invite>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = path_params.invite_id.to_string(),
        accept_invite = request.accept_invite,
        "Responding to project invitation"
    );

    // Find the invitation
    let Some(invite) = pg_client.find_invite_by_id(path_params.invite_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("project_invite")
            .with_message("Project invitation not found")
            .with_context(format!("Invite ID: {}", path_params.invite_id)));
    };

    // Verify invitation belongs to this project
    if invite.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound
            .with_resource("project_invite")
            .with_message("Project invitation not found in this project")
            .with_context(format!(
                "Expected project {}, but invite belongs to project {}",
                path_params.project_id, invite.project_id
            )));
    }

    // Verify invitation is valid
    if !invite.can_be_used() {
        return Err(ErrorKind::BadRequest
            .with_message("This invitation has expired or is no longer valid")
            .with_resource("project_invite")
            .with_context(format!(
                "Invite status: {:?}, Expires at: {}",
                invite.invite_status,
                invite.expires_at.to_jiff()
            )));
    }

    let project_invite = if request.accept_invite {
        // Accept the invitation
        let accepted_invite = pg_client
            .accept_invite(path_params.invite_id, auth_claims.account_id)
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            account_id = auth_claims.account_id.to_string(),
            project_id = path_params.project_id.to_string(),
            invite_id = path_params.invite_id.to_string(),
            "Project invitation accepted successfully"
        );

        accepted_invite
    } else {
        // Decline the invitation
        let declined_invite = pg_client
            .reject_invite(path_params.invite_id, auth_claims.account_id)
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            account_id = auth_claims.account_id.to_string(),
            project_id = path_params.project_id.to_string(),
            invite_id = path_params.invite_id.to_string(),
            "Project invitation declined"
        );

        declined_invite
    };

    Ok((StatusCode::OK, Json(Invite::from(project_invite))))
}

/// Generates a shareable invite code for a project.
///
/// Creates an invite code that can be shared with anyone to join the project.
/// The code can be used multiple times until it expires. Requires permission
/// to invite members.
#[tracing::instrument(skip_all)]
async fn generate_invite_code(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<GenerateInviteCode>,
) -> Result<(StatusCode, Json<InviteCode>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        project_id = %path_params.project_id,
        role = ?request.role,
        "Generating invite code"
    );

    // Verify user has permission to invite members
    auth_claims
        .authorize_project(
            &pg_client,
            path_params.project_id,
            Permission::InviteMembers,
        )
        .await?;

    // Generate expiration time based on the expiration option
    let expires_at = request.expires.to_expiry_timestamp();

    let new_invite = NewProjectInvite {
        project_id: path_params.project_id,
        invitee_id: None,
        invited_role: Some(request.role),
        expires_at: expires_at.map(Into::into),
        created_by: auth_claims.account_id,
        updated_by: auth_claims.account_id,
        ..Default::default()
    };

    let project_invite = pg_client.create_project_invite(new_invite).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        project_id = %path_params.project_id,
        invite_code = %project_invite.invite_token,
        "Invite code generated successfully"
    );

    Ok((
        StatusCode::CREATED,
        Json(InviteCode::from_invite(&project_invite)),
    ))
}

/// Joins a project using an invite code.
///
/// Allows a user to join a project by providing a valid invite code in the URL.
/// The user will be added as a member with the role specified when the
/// invite code was generated.
#[tracing::instrument(skip_all)]
async fn join_via_invite_code(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<InviteCodePathParams>,
) -> Result<(StatusCode, Json<Member>)> {
    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        "Attempting to join project via invite code"
    );

    // Find the invitation by token
    let Some(invite) = pg_client
        .find_invite_by_token(&path_params.invite_code)
        .await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("invite_code")
            .with_message("Invalid invite code")
            .with_context("The invite code does not exist or has been revoked"));
    };

    // Check if invite is still valid
    if !invite.can_be_used() {
        return Err(ErrorKind::BadRequest
            .with_message("This invite code has expired or is no longer valid")
            .with_resource("invite_code")
            .with_context(format!(
                "Status: {:?}, Expires at: {}",
                invite.invite_status,
                invite.expires_at.to_jiff()
            )));
    }

    // Check if user is already a member
    if let Some(existing_member) = pg_client
        .find_project_member(invite.project_id, auth_claims.account_id)
        .await?
        && existing_member.is_active
    {
        return Err(ErrorKind::Conflict
            .with_message("You are already a member of this project")
            .with_resource("project_member"));
    }

    // Add user as a project member
    let new_member = nvisy_postgres::model::NewProjectMember {
        project_id: invite.project_id,
        account_id: auth_claims.account_id,
        member_role: invite.invited_role,
        ..Default::default()
    };

    let project_member = pg_client.add_project_member(new_member).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        project_id = %invite.project_id,
        role = ?invite.invited_role,
        "User joined project via invite code"
    );

    Ok((StatusCode::CREATED, Json(Member::from(project_member))))
}

/// Returns a [`Router`] with all project invite related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/projects/:project_id/invites/", post(send_invite))
        .api_route("/projects/:project_id/invites/", get(list_invites))
        .api_route(
            "/projects/:project_id/invites/:invite_id/",
            delete(cancel_invite),
        )
        .api_route(
            "/projects/:project_id/invites/:invite_id/reply/",
            patch(reply_to_invite),
        )
        .api_route(
            "/projects/:project_id/invites/code/",
            post(generate_invite_code),
        )
        .api_route("/invites/:invite_code/join/", post(join_via_invite_code))
}
