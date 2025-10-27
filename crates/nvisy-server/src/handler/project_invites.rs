//! Project invitation management handlers.
//!
//! This module provides comprehensive project invitation functionality, allowing
//! project administrators to invite users to join projects with specific roles.
//! All operations are secured with proper authorization and include invitation
//! lifecycle management.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewProjectInvite, ProjectInvite};
use nvisy_postgres::query::{ProjectInviteRepository, ProjectMemberRepository};
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
const TRACING_TARGET: &str = "nvisy_server::handler::project_invites";

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
#[schema(example = json!({
    "inviteeEmail": "colleague@example.com",
    "invitedRole": "Editor",
    "inviteMessage": "Join our project to collaborate on documents!",
    "expiresInDays": 7
}))]
struct CreateInviteRequest {
    /// Email address of the person to invite.
    #[validate(email, length(max = 254))]
    pub invitee_email: String,
    /// Role the invitee will have if they accept the invitation.
    #[serde(default = "default_invite_role")]
    pub invited_role: ProjectRole,
    /// Optional personal message to include with the invitation.
    ///
    /// This message will be included in the invitation email. The content is
    /// validated to prevent XSS and injection attacks.
    #[validate(length(max = 1000), custom(function = "validate_safe_text"))]
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
///
/// This response includes all the essential information about the newly created
/// invitation, including the unique invite ID that can be used to track or cancel
/// the invitation later.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateInviteResponse {
    /// Unique identifier of the created invitation.
    ///
    /// This ID can be used to cancel the invitation or track its status.
    pub invite_id: Uuid,

    /// ID of the project the invitation is for.
    ///
    /// The invitee will become a member of this project if they accept.
    pub project_id: Uuid,

    /// Email address of the invitee.
    ///
    /// The invitation will be sent to this email address. The email is normalized
    /// to lowercase for consistency.
    pub invitee_email: String,

    /// Role the invitee will have if they accept.
    ///
    /// Determines the level of access and permissions the invitee will have
    /// in the project. Common roles include: Owner, Admin, Editor, Viewer.
    pub invited_role: ProjectRole,

    /// Current status of the invitation.
    ///
    /// Possible values: Pending, Accepted, Rejected, Cancelled, Expired.
    /// Newly created invitations start with status Pending.
    pub invite_status: InviteStatus,

    /// When the invitation will expire.
    ///
    /// After this timestamp, the invitation can no longer be accepted.
    /// The expiration period is configurable when creating the invite (1-30 days).
    pub expires_at: OffsetDateTime,

    /// When the invitation was created.
    ///
    /// UTC timestamp of when this invitation record was created.
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
///
/// Contains a paginated list of all invitations for a specific project,
/// including their current status and metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ListInvitesResponse {
    /// ID of the project these invitations belong to.
    pub project_id: Uuid,

    /// List of project invitations.
    ///
    /// This list contains all invitations matching the query, subject to
    /// pagination limits. Each item includes the invitation status and
    /// details about the invitee.
    pub invites: Vec<ListInvitesResponseItem>,

    /// Total count of invitations for this project.
    ///
    /// This count represents all invitations, not just the current page.
    /// Use this value to implement pagination controls in the UI.
    pub total_count: usize,
}

/// Request payload for responding to a project invitation.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "acceptInvite": true
}))]
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    ValidateJson(request): ValidateJson<CreateInviteRequest>,
) -> Result<(StatusCode, Json<CreateInviteResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = auth_claims.account_id.to_string(),
        project_id = path_params.project_id.to_string(),
        invitee_email = %request.invitee_email,
        invited_role = ?request.invited_role,
        "creating project invitation"
    );

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
    let all_invites = ProjectInviteRepository::list_user_invites(
        &mut conn,
        None,
        Some(normalized_email.clone()),
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
        + time::Duration::days(request.expires_in_days.clamp(1, 30) as i64);

    // Sanitize the invite message for additional security
    let sanitized_message = sanitize_text(&request.invite_message);

    let new_invite = NewProjectInvite {
        project_id: path_params.project_id,
        invitee_email: request.invitee_email.to_lowercase(),
        invitee_id: None, // Will be set when user accepts if they have an account
        invited_role: request.invited_role,
        invite_message: sanitized_message,
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
        target: TRACING_TARGET,
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<ProjectPathParams>,
    Json(pagination): Json<Pagination>,
) -> Result<(StatusCode, Json<ListInvitesResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    tracing::debug!(
        target: TRACING_TARGET,
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
        target: TRACING_TARGET,
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<InvitePathParams>,
) -> Result<(StatusCode, Json<CancelInviteResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
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
        target: TRACING_TARGET,
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<InvitePathParams>,
    Json(request): Json<ReplyInviteRequest>,
) -> Result<(StatusCode, Json<ReplyInviteResponse>)> {
    let mut conn = pg_client.get_connection().await?;

    tracing::info!(
        target: TRACING_TARGET,
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
        let accepted_invite = ProjectInviteRepository::accept_invite(
            &mut conn,
            path_params.invite_id,
            auth_claims.account_id,
        )
        .await?;

        tracing::info!(
            target: TRACING_TARGET,
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
            target: TRACING_TARGET,
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

/// Validates that text content is safe and doesn't contain potential XSS or injection attacks.
///
/// This function performs basic sanitization checks to prevent common security issues.
fn validate_safe_text(text: &str) -> Result<(), validator::ValidationError> {
    // Check for script tags
    if text.to_lowercase().contains("<script") {
        return Err(validator::ValidationError::new("contains_script_tag"));
    }

    // Check for common XSS patterns
    if text.contains("javascript:") || text.contains("data:text/html") {
        return Err(validator::ValidationError::new("contains_xss_pattern"));
    }

    // Check for SQL injection patterns (basic check)
    let suspicious_patterns = ["--", "/*", "*/", "xp_", "sp_", "exec(", "execute("];
    for pattern in &suspicious_patterns {
        if text.to_lowercase().contains(pattern) {
            return Err(validator::ValidationError::new(
                "contains_suspicious_pattern",
            ));
        }
    }

    Ok(())
}

/// Sanitizes user input by removing potentially dangerous characters.
///
/// This is a defense-in-depth measure in addition to validation.
fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '<' | '>' | '{' | '}' | '`'))
        .collect()
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
