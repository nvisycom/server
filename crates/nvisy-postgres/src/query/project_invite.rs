//! Project invite repository for managing project invitation operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectInvite, ProjectInvite, UpdateProjectInvite};
use crate::types::InviteStatus;
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive project invitation database operations.
///
/// Provides database operations for managing project invitations throughout
/// their lifecycle, from creation and delivery through acceptance, rejection,
/// and cleanup. This repository handles all database interactions related to
/// project member invitations and supports various invitation workflows.
///
/// The repository manages invitation tokens, expiration times, status tracking,
/// and provides comprehensive querying capabilities for building invitation
/// management interfaces and automated invitation processes.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectInviteRepository;

impl ProjectInviteRepository {
    /// Creates a new project invite repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `ProjectInviteRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new project invitation with secure token generation.
    ///
    /// Generates a new invitation record for the specified project with
    /// a unique invitation token and expiration time. The invitation is
    /// immediately available for delivery and acceptance by the invitee.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `invite` - Complete invitation data including project ID, invitee details, and role
    ///
    /// # Returns
    ///
    /// The created `ProjectInvite` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Security Considerations
    ///
    /// - Invitation tokens should be cryptographically secure
    /// - Expiration times should be reasonable for the invitation context
    /// - Consider rate limiting invitation creation to prevent abuse
    /// - Validate invitee permissions and project access rights
    pub async fn create_project_invite(
        conn: &mut AsyncPgConnection,
        invite: NewProjectInvite,
    ) -> PgResult<ProjectInvite> {
        use schema::project_invites;

        let invite = diesel::insert_into(project_invites::table)
            .values(&invite)
            .returning(ProjectInvite::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(invite)
    }

    /// Finds an invitation by its unique token string.
    ///
    /// Retrieves an invitation using its token for validation and acceptance
    /// workflows. This is the primary method used when users click invitation
    /// links or enter invitation codes. Returns the invitation regardless
    /// of its current status or expiration.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `token` - Invitation token string to search for
    ///
    /// # Returns
    ///
    /// The matching `ProjectInvite` if found, `None` if not found,
    /// or a database error if the query fails.
    ///
    /// # Invitation Flow Use Cases
    ///
    /// - Processing invitation link clicks
    /// - Validating invitation codes entered by users
    /// - Pre-populating invitation acceptance forms
    /// - Administrative invitation lookup and management
    pub async fn find_invite_by_token(
        conn: &mut AsyncPgConnection,
        token: &str,
    ) -> PgResult<Option<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invite = project_invites
            .filter(invite_token.eq(token))
            .select(ProjectInvite::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }

    /// Finds an invitation by its unique identifier.
    ///
    /// Retrieves a specific invitation using its UUID for direct access
    /// and administrative operations. This method returns the invitation
    /// regardless of its status, making it suitable for all invitation
    /// management scenarios.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `invite_id` - UUID of the invitation to retrieve
    ///
    /// # Returns
    ///
    /// The matching `ProjectInvite` if found, `None` if not found,
    /// or a database error if the query fails.
    ///
    /// # Administrative Use Cases
    ///
    /// - Direct invitation management and updates
    /// - Administrative invitation review and oversight
    /// - Invitation status tracking and reporting
    /// - Integration with invitation management interfaces
    pub async fn find_invite_by_id(
        conn: &mut AsyncPgConnection,
        invite_id: Uuid,
    ) -> PgResult<Option<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invite = project_invites
            .filter(id.eq(invite_id))
            .select(ProjectInvite::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }

    /// Updates a project invitation with new values and status changes.
    ///
    /// Applies partial updates to an existing invitation using the provided
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged. Commonly used for status
    /// updates, expiration changes, and administrative modifications.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `invite_id` - UUID of the invitation to update
    /// * `changes` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `ProjectInvite` with new values,
    /// or a database error if the operation fails.
    ///
    /// # Common Update Scenarios
    ///
    /// - Changing invitation status (accepted, declined, expired)
    /// - Extending or modifying expiration times
    /// - Adding response timestamps and user tracking
    /// - Administrative invitation adjustments
    pub async fn update_project_invite(
        conn: &mut AsyncPgConnection,
        invite_id: Uuid,
        changes: UpdateProjectInvite,
    ) -> PgResult<ProjectInvite> {
        use schema::project_invites::dsl::*;

        let invite = diesel::update(project_invites)
            .filter(id.eq(invite_id))
            .set(&changes)
            .returning(ProjectInvite::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(invite)
    }

    /// Accepts a project invitation and marks it as successfully processed.
    ///
    /// Updates the invitation status to Accepted and sets the response
    /// timestamp. This method should be called after successfully adding
    /// the user to the project to maintain accurate invitation tracking
    /// and prevent duplicate processing.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `invite_id` - UUID of the invitation to accept
    /// * `_acceptor_id` - UUID of the user accepting the invitation (for audit trails)
    ///
    /// # Returns
    ///
    /// The updated `ProjectInvite` with accepted status and response timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Integration with Project Membership
    ///
    /// This method only updates the invitation status. The caller is
    /// responsible for actually adding the user to the project membership
    /// and handling any related setup or notification processes.
    pub async fn accept_invite(
        conn: &mut AsyncPgConnection,
        invite_id: Uuid,
        _acceptor_id: Uuid,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Accepted),
            responded_at: Some(OffsetDateTime::now_utc()),
            ..Default::default()
        };

        Self::update_project_invite(conn, invite_id, changes).await
    }

    /// Rejects or declines a project invitation.
    ///
    /// Updates the invitation status to Declined when a user chooses
    /// not to accept the invitation. This provides a clear audit trail
    /// of invitation responses and prevents the invitation from being
    /// used in the future.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `invite_id` - UUID of the invitation to reject
    /// * `updated_by_id` - UUID of the user declining the invitation
    ///
    /// # Returns
    ///
    /// The updated `ProjectInvite` with declined status,
    /// or a database error if the operation fails.
    ///
    /// # User Experience Benefits
    ///
    /// - Provides clear feedback on invitation responses
    /// - Prevents future confusion about invitation status
    /// - Enables invitation analytics and response tracking
    /// - Supports user preference management
    pub async fn reject_invite(
        conn: &mut AsyncPgConnection,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Declined),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        Self::update_project_invite(conn, invite_id, changes).await
    }

    /// Cancels a project invitation before it can be used.
    ///
    /// Updates the invitation status to Canceled, typically used when
    /// the inviter or an administrator decides to revoke the invitation
    /// before it has been responded to. This prevents the invitation
    /// from being accepted and maintains a clear audit trail.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `invite_id` - UUID of the invitation to cancel
    /// * `updated_by_id` - UUID of the user canceling the invitation
    ///
    /// # Returns
    ///
    /// The updated `ProjectInvite` with canceled status,
    /// or a database error if the operation fails.
    ///
    /// # Cancellation Use Cases
    ///
    /// - Correcting mistaken invitations
    /// - Responding to changed project requirements
    /// - Administrative invitation management
    /// - Security incident response
    pub async fn cancel_invite(
        conn: &mut AsyncPgConnection,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Canceled),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        Self::update_project_invite(conn, invite_id, changes).await
    }

    /// Lists all invitations for a specific project with pagination support.
    ///
    /// Retrieves a paginated list of all invitations associated with
    /// the specified project, regardless of their status. Results are
    /// ordered by creation date with newest invitations first, providing
    /// a comprehensive view of the project's invitation history.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to list invitations for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of all `ProjectInvite` entries for the project,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # Project Management Use Cases
    ///
    /// - Project member management dashboards
    /// - Invitation history and audit trails
    /// - Administrative project oversight
    /// - Invitation analytics and reporting
    pub async fn list_project_invites(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invites = project_invites
            .filter(project_id.eq(proj_id))
            .select(ProjectInvite::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    /// Lists invitations for a specific user with pagination support.
    ///
    /// Retrieves a paginated list of invitations for the specified user,
    /// enabling users to see all their pending and historical invitations
    /// across all projects. Results are ordered by creation date with
    /// newest invitations first.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `user_id` - Optional UUID of the user to list invitations for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectInvite` entries for the user,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # User Dashboard Use Cases
    ///
    /// - User invitation inbox and management
    /// - "Invitations" section in user dashboards
    /// - User notification and alert systems
    /// - Cross-project invitation tracking
    pub async fn list_user_invites(
        conn: &mut AsyncPgConnection,
        user_id: Option<Uuid>,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let mut query = project_invites.into_boxed();

        if let Some(uid) = user_id {
            query = query.filter(invitee_id.eq(uid));
        }

        let invites = query
            .select(ProjectInvite::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    /// Performs system-wide cleanup of expired invitations.
    ///
    /// Updates all pending invitations that have passed their expiration
    /// time to Expired status. This maintenance operation should be run
    /// regularly to keep invitation data current and prevent users from
    /// attempting to use expired invitations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    ///
    /// # Returns
    ///
    /// The number of invitations that were marked as expired,
    /// or a database error if the operation fails.
    ///
    /// # Maintenance Benefits
    ///
    /// - Keeps invitation status accurate and current
    /// - Improves user experience by preventing expired invitation attempts
    /// - Maintains database hygiene and performance
    /// - Should be automated via scheduled maintenance jobs
    ///
    /// # Scheduling Recommendation
    ///
    /// Run this operation hourly or daily depending on invitation
    /// volume and expiration patterns to maintain optimal user experience.
    pub async fn cleanup_expired_invites(conn: &mut AsyncPgConnection) -> PgResult<usize> {
        use schema::project_invites::dsl::*;

        let now = OffsetDateTime::now_utc();

        let updated_count = diesel::update(project_invites)
            .filter(expires_at.lt(now))
            .filter(invite_status.eq(InviteStatus::Pending))
            .set(invite_status.eq(InviteStatus::Expired))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(updated_count)
    }

    /// Retrieves all pending invitations for a specific project.
    ///
    /// Returns invitations that are still awaiting response and haven't
    /// expired yet. This provides a focused view of active invitations
    /// that may still result in new project members, useful for project
    /// management and invitation tracking.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to get pending invitations for
    ///
    /// # Returns
    ///
    /// A vector of pending, non-expired `ProjectInvite` entries,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # Active Invitation Management Use Cases
    ///
    /// - Project dashboard invitation status displays
    /// - Calculating expected project growth
    /// - Invitation follow-up and reminder systems
    /// - Active member recruitment tracking
    pub async fn get_pending_invites(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invites = project_invites
            .filter(project_id.eq(proj_id))
            .filter(invite_status.eq(InviteStatus::Pending))
            .filter(expires_at.gt(OffsetDateTime::now_utc()))
            .select(ProjectInvite::as_select())
            .order(created_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    /// Finds invitations filtered by their current status.
    ///
    /// Retrieves a paginated list of invitations with the specified status
    /// across all projects. This enables status-specific invitation
    /// management, analytics, and administrative oversight of the
    /// invitation system.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `status` - Invitation status to filter by (Pending, Accepted, Declined, etc.)
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectInvite` entries with the specified status,
    /// ordered by creation date (newest first), or a database error if the query fails.
    ///
    /// # Administrative and Analytics Use Cases
    ///
    /// - System-wide invitation status monitoring
    /// - Invitation success rate analysis
    /// - Bulk invitation management operations
    /// - Administrative reporting and insights
    pub async fn find_invites_by_status(
        conn: &mut AsyncPgConnection,
        status: InviteStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invites = project_invites
            .filter(invite_status.eq(status))
            .select(ProjectInvite::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    /// Finds invitations that are approaching their expiration time.
    ///
    /// Identifies pending invitations that will expire within the specified
    /// time window. This is useful for proactive reminder systems,
    /// invitation extension workflows, and preventing invitation expiration
    /// without user awareness.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `hours` - Number of hours ahead to look for expiring invitations
    ///
    /// # Returns
    ///
    /// A vector of `ProjectInvite` entries expiring within the time window,
    /// ordered by expiration time (soonest first), or a database error if the query fails.
    ///
    /// # Proactive Management Use Cases
    ///
    /// - Automated reminder email systems
    /// - Invitation extension workflows
    /// - User notification and alert systems
    /// - Preventing invitation expiration without user action
    pub async fn find_expiring_invites(
        conn: &mut AsyncPgConnection,
        hours: i64,
    ) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let expiry_threshold = OffsetDateTime::now_utc() + time::Duration::hours(hours);

        let invites = project_invites
            .filter(invite_status.eq(InviteStatus::Pending))
            .filter(expires_at.between(OffsetDateTime::now_utc(), expiry_threshold))
            .select(ProjectInvite::as_select())
            .order(expires_at.asc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    /// Revokes an invitation through administrative action.
    ///
    /// Updates the invitation status to Revoked, typically used for
    /// security incidents, policy violations, or administrative
    /// intervention. This prevents the invitation from being used
    /// and provides a clear audit trail of administrative actions.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `invite_id` - UUID of the invitation to revoke
    /// * `updated_by_id` - UUID of the administrator revoking the invitation
    /// * `_reason` - Optional reason for revocation (for audit purposes)
    ///
    /// # Returns
    ///
    /// The updated `ProjectInvite` with revoked status,
    /// or a database error if the operation fails.
    ///
    /// # Administrative Use Cases
    ///
    /// - Security incident response
    /// - Policy violation enforcement
    /// - Correcting invitation errors
    /// - Compliance with organizational policies
    pub async fn revoke_invite(
        conn: &mut AsyncPgConnection,
        invite_id: Uuid,
        updated_by_id: Uuid,
        _reason: Option<String>,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Revoked),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        Self::update_project_invite(conn, invite_id, changes).await
    }

    /// Retrieves an invitation by its unique identifier.
    ///
    /// This method is an alias for `find_invite_by_id` and provides
    /// the same functionality with a different naming convention.
    /// Returns the invitation regardless of its current status.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `invite_id` - UUID of the invitation to retrieve
    ///
    /// # Returns
    ///
    /// The matching `ProjectInvite` if found, `None` if not found,
    /// or a database error if the query fails.
    pub async fn get_invite_by_id(
        conn: &mut AsyncPgConnection,
        invite_id: Uuid,
    ) -> PgResult<Option<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invite = project_invites
            .filter(id.eq(invite_id))
            .select(ProjectInvite::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }
}
