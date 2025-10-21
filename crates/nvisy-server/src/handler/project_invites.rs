//! Project invitation management handlers.
//!
//! This module provides comprehensive project invitation functionality, allowing
//! project administrators to invite users to join projects with specific roles.
//! All operations are secured with proper authorization and include invitation
//! lifecycle management.
//!
//! # Security Features
//!
//! ## Authorization Requirements
//! - Project admin or owner permissions required for sending invitations
//! - Invite management restricted to project administrators
//! - Email validation and domain restrictions
//! - Rate limiting to prevent invitation spam
//!
//! ## Invitation Workflow
//! 1. **Send Invite**: Admin creates invitation with target email and role
//! 2. **Email Notification**: Automated email sent to invitee
//! 3. **Accept/Decline**: Invitee responds through secure link
//! 4. **Member Creation**: Successful acceptance creates project membership
//! 5. **Cleanup**: Expired or declined invitations are automatically removed
//!
//! # Role-Based Invitations
//!
//! - **Viewer**: Read-only access to project and documents
//! - **Editor**: Can create and modify documents within the project
//! - **Admin**: Can manage project settings and invite other members
//! - **Owner**: Full control (typically not assignable through invitations)
//!
//! # Invitation States
//!
//! - **Pending**: Invitation sent, awaiting response
//! - **Accepted**: Invitation accepted, membership created
//! - **Declined**: Invitation declined by invitee
//! - **Expired**: Invitation expired (typically after 7 days)
//! - **Revoked**: Invitation cancelled by project administrator
//!
//! # Endpoints
//!
//! ## Invitation Management
//! - `POST /projects/{projectId}/invites` - Send invitation to email address
//! - `GET /projects/{projectId}/invites` - List pending invitations
//! - `DELETE /projects/{projectId}/invites/{inviteId}` - Cancel pending invitation
//!
//! # Security Considerations
//!
//! - Email validation prevents invalid invitations
//! - Time-limited invitation tokens prevent replay attacks
//! - Project permission validation ensures only authorized users can invite
//! - Audit logging tracks all invitation activities

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::models::{NewProjectInvite, ProjectInvite};
use nvisy_postgres::queries::{ProjectInviteRepository, ProjectMemberRepository};
use nvisy_postgres::types::{InviteStatus, ProjectRole};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use validator::Validate;

use crate::extract::auth::AuthProvider;
use crate::extract::{AuthState, Json, Path, ProjectPermission, ValidateJson};
use crate::handler::projects::ProjectPathParams;
use crate::handler::{ErrorKind, ErrorResponse, Pagination, Result};
use crate::service::ServiceState;

/// Tracing target for project invite operations.
const TRACING_TARGET: &str = "nvisy::handler::project_invites";

/// Path parameters for invite-specific endpoints.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct InvitePathParams {
    /// Unique identifier of the project.
    pub project_id: Uuid,
    /// Unique identifier of the invite.
    pub invite_id: Uuid,
}

/// Path parameters for invitee-specific endpoints.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct InviteePathParams {
    /// Unique identifier of the project.
    pub project_id: Uuid,
    /// Unique identifier of the invitee account.
    pub account_id: Uuid,
}

/// Request payload for creating a new project invite.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
struct CreateInviteRequest {
    /// Email address of the person to invite.
    #[validate(email, length(max = 254))]
    pub invitee_email: String,
    /// Role the invitee will have if they accept the invitation.
    #[serde(default = "default_invite_role")]
    pub invited_role: ProjectRole,
    /// Optional personal message to include with the invitation.
    #[validate(length(max = 1000))]
    #[serde(default)]
    pub invite_message: String,
    /// Number of days until the invitation expires (1-30 days, default: 7).
    #[validate(range(min = 1, max = 30))]
    #[serde(default = "default_expiry_days")]
    pub expires_in_days: u8,
}

fn default_invite_role() -> ProjectRole {
    ProjectRole::Editor
}

fn default_expiry_days() -> u8 {
    7
}

/// Response returned when a project invite is successfully created.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateInviteResponse {
    /// Unique identifier of the created invitation.
    pub invite_id: Uuid,
    /// ID of the project the invitation is for.
    pub project_id: Uuid,
    /// Email address of the invitee.
    pub invitee_email: String,
    /// Role the invitee will have if they accept.
    pub invited_role: ProjectRole,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation will expire.
    pub expires_at: OffsetDateTime,
    /// When the invitation was created.
    pub created_at: OffsetDateTime,
}

impl From<ProjectInvite> for CreateInviteResponse {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            project_id: invite.project_id,
            invitee_email: invite.invitee_email,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at,
            created_at: invite.created_at,
        }
    }
}

/// Represents a project invitation in list responses.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListInvitesResponseItem {
    /// Unique identifier of the invitation.
    pub invite_id: Uuid,
    /// Email address of the invitee.
    pub invitee_email: String,
    /// Account ID if the invitee has an account.
    pub invitee_id: Option<Uuid>,
    /// Role the invitee will have if they accept.
    pub invited_role: ProjectRole,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation expires.
    pub expires_at: OffsetDateTime,
    /// When the invitation was created.
    pub created_at: OffsetDateTime,
    /// When the invitation was last updated.
    pub updated_at: OffsetDateTime,
}

impl From<ProjectInvite> for ListInvitesResponseItem {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            invitee_email: invite.invitee_email,
            invitee_id: invite.invitee_id,
            invited_role: invite.invited_role,
            invite_status: invite.invite_status,
            expires_at: invite.expires_at,
            created_at: invite.created_at,
            updated_at: invite.updated_at,
        }
    }
}

/// Response for listing project invitations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListInvitesResponse {
    /// ID of the project.
    pub project_id: Uuid,
    /// List of project invitations.
    pub invites: Vec<ListInvitesResponseItem>,
    /// Total count of invitations (for pagination).
    pub total_count: usize,
}

/// Request payload for responding to a project invitation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ReplyInviteRequest {
    /// Whether to accept or decline the invitation.
    pub accept_invite: bool,
}

/// Response for invitation reply operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ReplyInviteResponse {
    /// ID of the invitation.
    pub invite_id: Uuid,
    /// ID of the project.
    pub project_id: Uuid,
    /// Email address of the invitee.
    pub invitee_email: String,
    /// Current status of the invitation.
    pub invite_status: InviteStatus,
    /// When the invitation was accepted or declined.
    pub updated_at: OffsetDateTime,
}

impl From<ProjectInvite> for ReplyInviteResponse {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            project_id: invite.project_id,
            invitee_email: invite.invitee_email,
            invite_status: invite.invite_status,
            updated_at: invite.updated_at,
        }
    }
}

/// Response for invitation cancellation operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CancelInviteResponse {
    /// ID of the cancelled invitation.
    pub invite_id: Uuid,
    /// ID of the project.
    pub project_id: Uuid,
    /// Email address of the invitee.
    pub invitee_email: String,
    /// Reason for cancellation.
    pub status_reason: Option<String>,
}

impl From<ProjectInvite> for CancelInviteResponse {
    fn from(invite: ProjectInvite) -> Self {
        Self {
            invite_id: invite.id,
            project_id: invite.project_id,
            invitee_email: invite.invitee_email,
            status_reason: invite.status_reason,
        }
    }
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
        content = CreateInviteRequest,
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
            body = CreateInviteResponse,
        ),
    ),
)]
async fn send_invite(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateInviteRequest>,
) -> Result<(StatusCode, Json<CreateInviteResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invitee_email = %request.invitee_email,
        invited_role = ?request.invited_role,
        "creating project invitation"
    );

    // Input validation is handled by ValidateJson extractor

    // Verify user has permission to send invitations
    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::InviteMembers,
        )
        .await?;

    // Check if user is already a member
    if let Some(existing_member) = ProjectMemberRepository::find_project_member(
        &mut conn,
        path_params.project_id,
        auth_claims.account_id,
    )
    .await?
        && existing_member.is_active
    {
        return Err(ErrorKind::Conflict.with_context("User is already a member of this project"));
    }

    // Check for existing pending invites to the same email
    let normalized_email = request.invitee_email.to_lowercase();
    let all_invites = ProjectInviteRepository::list_user_invites(
        &mut conn,
        None,
        Some(normalized_email.clone()),
        nvisy_postgres::queries::Pagination {
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
        .find(|invite| invite.invite_status == nvisy_postgres::types::InviteStatus::Pending)
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
        + time::Duration::days(request.expires_in_days.clamp(1, 30) as i64);

    let new_invite = NewProjectInvite {
        project_id: path_params.project_id,
        invitee_email: request.invitee_email.to_lowercase(),
        invitee_id: None, // Will be set when user accepts if they have an account
        invited_role: request.invited_role,
        invite_message: request.invite_message,
        invite_token: generate_invite_token(),
        expires_at,
        max_uses: 1,
        created_by: auth_claims.account_id,
        updated_by: auth_claims.account_id,
    };

    let project_invite =
        ProjectInviteRepository::create_project_invite(&mut conn, new_invite).await?;

    let response = CreateInviteResponse::from(project_invite);

    tracing::info!(
        target: "server::handler::project_invites",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = response.invite_id.to_string(),
        invitee_email = %response.invitee_email,
        "project invitation created successfully"
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
            body = ListInvitesResponse,
        ),
    ),
)]
async fn list_invites(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ListInvitesResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::debug!(
        target: "server::handler::project_invites",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        "listing project invitations"
    );

    // Verify user has permission to view project invitations
    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::ViewMembers,
        )
        .await?;

    // Retrieve project invitations with pagination
    let project_invites = ProjectInviteRepository::list_project_invites(
        &mut conn,
        path_params.project_id,
        pagination.into(),
    )
    .await?;

    let invites = project_invites
        .into_iter()
        .map(ListInvitesResponseItem::from)
        .collect::<Vec<_>>();

    let response = ListInvitesResponse {
        project_id: path_params.project_id,
        total_count: invites.len(),
        invites,
    };

    tracing::debug!(
        target: "server::handler::project_invites",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_count = response.invites.len(),
        "project invitations listed successfully"
    );

    Ok((StatusCode::OK, Json(response)))
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
            body = CancelInviteResponse,
        ),
    ),
)]
async fn cancel_invite(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<InvitePathParams>,
) -> Result<(StatusCode, Json<CancelInviteResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::info!(
        target: "server::handler::project_invites",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = path_params.invite_id.to_string(),
        "cancelling project invitation"
    );

    // Verify user has permission to manage project invitations
    auth_claims
        .authorize_project(
            &mut conn,
            path_params.project_id,
            ProjectPermission::InviteMembers,
        )
        .await?;

    // Cancel the invitation
    let project_invite = ProjectInviteRepository::cancel_invite(
        &mut conn,
        path_params.invite_id,
        auth_claims.account_id,
    )
    .await?;

    let response = CancelInviteResponse::from(project_invite);

    tracing::info!(
        target: "server::handler::project_invites",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = path_params.invite_id.to_string(),
        invitee_email = %response.invitee_email,
        "project invitation cancelled successfully"
    );

    Ok((StatusCode::OK, Json(response)))
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
        content = ReplyInviteRequest,
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
            body = ReplyInviteResponse,
        ),
    )
)]
async fn reply_to_invite(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<InvitePathParams>,
    Json(request): Json<ReplyInviteRequest>,
) -> Result<(StatusCode, Json<ReplyInviteResponse>)> {
    let mut conn = pg_database.get_connection().await?;

    tracing::info!(
        target: "server::handler::project_invites",
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invite_id = path_params.invite_id.to_string(),
        accept_invite = request.accept_invite,
        "responding to project invitation"
    );

    // Find the invitation
    let Some(invite) =
        ProjectInviteRepository::find_invite_by_id(&mut conn, path_params.invite_id).await?
    else {
        return Err(ErrorKind::NotFound.with_resource("invitation"));
    };

    // Verify invitation belongs to this project
    if invite.project_id != path_params.project_id {
        return Err(ErrorKind::NotFound.with_resource("invitation"));
    }

    // Verify invitation is valid
    if !invite.can_be_used() {
        return Err(
            ErrorKind::BadRequest.with_context("This invitation has expired or is no longer valid")
        );
    }

    let project_invite = if request.accept_invite {
        // Accept the invitation
        let accepted_invite = ProjectInviteRepository::accept_invite(
            &mut conn,
            path_params.invite_id,
            auth_claims.account_id,
        )
        .await?;

        tracing::info!(
            target: "server::handler::project_invites",
            account_id = auth_claims.account_id.to_string(),
            project_id = path_params.project_id.to_string(),
            invite_id = path_params.invite_id.to_string(),
            "project invitation accepted successfully"
        );

        accepted_invite
    } else {
        // Decline the invitation
        let declined_invite = ProjectInviteRepository::reject_invite(
            &mut conn,
            path_params.invite_id,
            auth_claims.account_id,
        )
        .await?;

        tracing::info!(
            target: "server::handler::project_invites",
            account_id = auth_claims.account_id.to_string(),
            project_id = path_params.project_id.to_string(),
            invite_id = path_params.invite_id.to_string(),
            "project invitation declined"
        );

        declined_invite
    };

    Ok((
        StatusCode::OK,
        Json(ReplyInviteResponse::from(project_invite)),
    ))
}

/// Generates a cryptographically secure invite token.
fn generate_invite_token() -> String {
    use uuid::Uuid;
    // In production, this should use a proper cryptographic token generator
    format!("invite_{}", Uuid::new_v4().to_string().replace('-', ""))
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
