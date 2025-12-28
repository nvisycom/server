//! Project invite repository for managing project invitation operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectInvite, ProjectInvite, UpdateProjectInvite};
use crate::types::InviteStatus;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for project invitation database operations.
///
/// Handles project invitations including creation, acceptance, rejection, and token
/// management with expiration tracking.
pub trait ProjectInviteRepository {
    /// Creates a new project invitation with secure token generation.
    fn create_project_invite(
        &mut self,
        invite: NewProjectInvite,
    ) -> impl Future<Output = PgResult<ProjectInvite>> + Send;

    /// Finds an invitation by its unique token string.
    fn find_invite_by_token(
        &mut self,
        token: &str,
    ) -> impl Future<Output = PgResult<Option<ProjectInvite>>> + Send;

    /// Finds an invitation by its unique identifier.
    fn find_invite_by_id(
        &mut self,
        invite_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectInvite>>> + Send;

    /// Updates a project invitation with new values and status changes.
    fn update_project_invite(
        &mut self,
        invite_id: Uuid,
        changes: UpdateProjectInvite,
    ) -> impl Future<Output = PgResult<ProjectInvite>> + Send;

    /// Accepts a project invitation and marks it as successfully processed.
    fn accept_invite(
        &mut self,
        invite_id: Uuid,
        _acceptor_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectInvite>> + Send;

    /// Rejects or declines a project invitation.
    fn reject_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectInvite>> + Send;

    /// Cancels a project invitation before it can be used.
    fn cancel_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectInvite>> + Send;

    /// Lists all invitations for a specific project with pagination support.
    fn list_project_invites(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectInvite>>> + Send;

    /// Lists invitations for a specific user with pagination support.
    fn list_user_invites(
        &mut self,
        user_id: Option<Uuid>,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectInvite>>> + Send;

    /// Performs system-wide cleanup of expired invitations.
    fn cleanup_expired_invites(&mut self) -> impl Future<Output = PgResult<usize>> + Send;

    /// Retrieves all pending invitations for a specific project.
    fn get_pending_invites(
        &mut self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ProjectInvite>>> + Send;

    /// Finds invitations filtered by their current status.
    fn find_invites_by_status(
        &mut self,
        status: InviteStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectInvite>>> + Send;

    /// Finds invitations that are approaching their expiration time.
    fn find_expiring_invites(
        &mut self,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<ProjectInvite>>> + Send;

    /// Revokes an invitation through administrative action.
    fn revoke_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
        _reason: Option<String>,
    ) -> impl Future<Output = PgResult<ProjectInvite>> + Send;

    /// Retrieves an invitation by its unique identifier.
    fn get_invite_by_id(
        &mut self,
        invite_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectInvite>>> + Send;
}

impl ProjectInviteRepository for PgConnection {
    async fn create_project_invite(&mut self, invite: NewProjectInvite) -> PgResult<ProjectInvite> {
        use schema::project_invites;

        let invite = diesel::insert_into(project_invites::table)
            .values(&invite)
            .returning(ProjectInvite::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn find_invite_by_token(&mut self, token: &str) -> PgResult<Option<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invite = project_invites
            .filter(invite_token.eq(token))
            .select(ProjectInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn find_invite_by_id(&mut self, invite_id: Uuid) -> PgResult<Option<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invite = project_invites
            .filter(id.eq(invite_id))
            .select(ProjectInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn update_project_invite(
        &mut self,
        invite_id: Uuid,
        changes: UpdateProjectInvite,
    ) -> PgResult<ProjectInvite> {
        use schema::project_invites::dsl::*;

        let invite = diesel::update(project_invites)
            .filter(id.eq(invite_id))
            .set(&changes)
            .returning(ProjectInvite::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn accept_invite(
        &mut self,
        invite_id: Uuid,
        _acceptor_id: Uuid,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Accepted),
            responded_at: Some(jiff_diesel::Timestamp::from(Timestamp::now())),
            ..Default::default()
        };

        self.update_project_invite(invite_id, changes).await
    }

    async fn reject_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Declined),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_project_invite(invite_id, changes).await
    }

    async fn cancel_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Canceled),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_project_invite(invite_id, changes).await
    }

    async fn list_project_invites(
        &mut self,
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
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn list_user_invites(
        &mut self,
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
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn cleanup_expired_invites(&mut self) -> PgResult<usize> {
        use schema::project_invites::dsl::*;

        let now = jiff_diesel::Timestamp::from(Timestamp::now());

        let updated_count = diesel::update(project_invites)
            .filter(expires_at.lt(now))
            .filter(invite_status.eq(InviteStatus::Pending))
            .set(invite_status.eq(InviteStatus::Expired))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(updated_count)
    }

    async fn get_pending_invites(&mut self, proj_id: Uuid) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invites = project_invites
            .filter(project_id.eq(proj_id))
            .filter(invite_status.eq(InviteStatus::Pending))
            .filter(expires_at.gt(jiff_diesel::Timestamp::from(Timestamp::now())))
            .select(ProjectInvite::as_select())
            .order(created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn find_invites_by_status(
        &mut self,
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
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn find_expiring_invites(&mut self, hours: i64) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let expiry_threshold =
            jiff_diesel::Timestamp::from(Timestamp::now() + Span::new().hours(hours));

        let invites = project_invites
            .filter(invite_status.eq(InviteStatus::Pending))
            .filter(expires_at.between(
                jiff_diesel::Timestamp::from(Timestamp::now()),
                expiry_threshold,
            ))
            .select(ProjectInvite::as_select())
            .order(expires_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn revoke_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
        _reason: Option<String>,
    ) -> PgResult<ProjectInvite> {
        let changes = UpdateProjectInvite {
            invite_status: Some(InviteStatus::Revoked),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_project_invite(invite_id, changes).await
    }

    async fn get_invite_by_id(&mut self, invite_id: Uuid) -> PgResult<Option<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let invite = project_invites
            .filter(id.eq(invite_id))
            .select(ProjectInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }
}
