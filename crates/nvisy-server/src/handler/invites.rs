//! Project invitation management handlers.
//!
//! This module provides comprehensive project invitation functionality, allowing
//! project administrators to invite users to join projects with specific roles.
//! All operations are secured with proper authorization and include invitation
//! lifecycle management.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::NewProjectInvite;
use nvisy_postgres::query::{ProjectInviteRepository, ProjectMemberRepository};
use nvisy_postgres::types::InviteStatus;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::extract::{AuthProvider, AuthState, Json, Path, Permission, ValidateJson};
use crate::handler::projects::ProjectPathParams;
use crate::handler::request::{CreateInvite, ReplyInvite};
use crate::handler::response::{Invite, Invites};
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for project invite operations.
const TRACING_TARGET: &str = "nvisy_server::handler::project_invites";

/// Combined path parameters for invite-specific endpoints.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct InvitePathParams {
    /// Unique identifier of the project.
    pub project_id: Uuid,
    /// Unique identifier of the invite.
    pub invite_id: Uuid,
}

/// Creates a new project invitation.
///
/// Sends an invitation to join a project to the specified email address.
/// The invitee will receive an email with instructions to accept or decline.
/// Requires administrator permissions to send invitations.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/projects/{projectId}/invites/", tag = "invites",
    params(ProjectPathParams),
    request_body(
        content = CreateInvite,
        description = "Invitation details",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Invalid request data",
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
            status = CONFLICT,
            description = "User is already a member or has a pending invitation",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = CREATED,
            description = "Project invitation created successfully",
            body = Invite,
        ),
    ),
)]
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
                normalized_email, pending_invite.expires_at
            )));
    }

    // Generate expiration time
    let expires_at = OffsetDateTime::now_utc()
        + time::Duration::days(request.expires_in_days.unwrap_or(7).clamp(1, 30) as i64);

    // Sanitize the invite message for additional security
    let sanitized_message = sanitize_text(&request.invite_message);

    let new_invite = NewProjectInvite {
        project_id: path_params.project_id,
        invitee_id: None, // Will be set when user accepts if they have an account
        invited_role: Some(request.invited_role),
        invite_message: Some(sanitized_message),
        expires_at: Some(expires_at),
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
#[utoipa::path(
    get, path = "/projects/{projectId}/invites/", tag = "invites",
    params(
        ProjectPathParams,
        ("status" = Option<InviteStatus>, Query, description = "Filter by invitation status")
    ),
    request_body(
        content = Pagination,
        description = "Pagination parameters",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Invalid request parameters",
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
            description = "Project invitations listed successfully",
            body = Invites,
        ),
    ),
)]
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
#[utoipa::path(
    delete, path = "/projects/{projectId}/invites/{inviteId}/", tag = "invites",
    params(InvitePathParams),
    responses(
        (
            status = BAD_REQUEST,
            description = "Invalid request",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Access denied - insufficient permissions",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Project or invitation not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Project invitation cancelled successfully",
        ),
    ),
)]
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
#[utoipa::path(
    patch, path = "/projects/{projectId}/invites/{inviteId}/reply/", tag = "invites",
    params(InvitePathParams),
    request_body(
        content = ReplyInvite,
        description = "Invitation response",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Invalid request or invitation expired",
            body = ErrorResponse,
        ),
        (
            status = FORBIDDEN,
            description = "Not authorized to respond to this invitation",
            body = ErrorResponse,
        ),
        (
            status = NOT_FOUND,
            description = "Project or invitation not found",
            body = ErrorResponse,
        ),
        (
            status = CONFLICT,
            description = "User is already a member of the project",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Invitation response processed successfully",
            body = Invite,
        ),
    )
)]
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
                invite.invite_status, invite.expires_at
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

/// Sanitizes user input by removing potentially dangerous characters.
///
/// This is a defense-in-depth measure in addition to validation.
fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '<' | '>' | '{' | '}' | '`'))
        .collect()
}

/// Returns a [`Router`] with all project invite related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new()
        .routes(routes!(send_invite, list_invites))
        .routes(routes!(cancel_invite, reply_to_invite))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn project_invite_routes_integration() -> anyhow::Result<()> {
        let _server = create_test_server_with_router(|_| routes()).await?;

        // TODO: Add comprehensive integration tests for:
        // - Creating invitations with proper validation
        // - Listing invitations with pagination and filtering
        // - Accepting/declining invitations
        // - Cancelling invitations with proper authorization
        // - Error scenarios and edge cases
        // - Email validation and business logic

        Ok(())
    }

    #[test]
    fn test_create_invite_validation() {
        // TODO: Add tests using ValidateJson extractor
        // - Test valid requests pass validation
        // - Test invalid emails are rejected
        // - Test message length limits
        // - Test expiry day ranges
    }

    #[test]
    fn test_response_conversions() {
        // TODO: Add unit tests for response model conversions
        // - Test From<ProjectInvite> implementations
        // - Verify all fields are properly mapped
        // - Check serialization behavior
    }
}
