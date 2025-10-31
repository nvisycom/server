//! Project invite repository for managing project invitation operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectInvite, ProjectInvite, UpdateProjectInvite};
use crate::types::InviteStatus;
use crate::{PgError, PgResult, schema};

/// Repository for project invite table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectInviteRepository;

impl ProjectInviteRepository {
    /// Creates a new project invite repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new project invitation.
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

    /// Finds an invitation by its token.
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

    /// Finds an invitation by its ID.
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

    /// Updates a project invitation.
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

    /// Accepts a project invitation.
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

    /// Rejects/declines a project invitation.
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

    /// Cancels a project invitation.
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

    /// Lists all invitations for a project.
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

    /// Lists invitations for a specific user (by email or account ID).
    pub async fn list_user_invites(
        conn: &mut AsyncPgConnection,
        user_id: Option<Uuid>,
        email: Option<String>,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectInvite>> {
        use schema::project_invites::dsl::*;

        let mut query = project_invites.into_boxed();

        if let Some(uid) = user_id {
            query = query.filter(invitee_id.eq(uid));
        }

        if let Some(user_email) = email {
            query = query.filter(invitee_email.eq(user_email));
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

    /// Cleans up expired invitations.
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

    /// Gets pending invitations for a project.
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

    /// Gets invitations by status.
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

    /// Finds invitations that are about to expire (within specified hours).
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

    /// Revokes an invitation (admin action).
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

    /// Gets an invitation by ID (use_count field no longer exists).
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

    /// Gets invitation statistics for a project.
    pub async fn get_invite_stats(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<(i64, i64, i64, i64)> {
        use schema::project_invites::dsl::*;

        // Count pending invites
        let pending_count: i64 = project_invites
            .filter(project_id.eq(proj_id))
            .filter(invite_status.eq(InviteStatus::Pending))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count accepted invites
        let accepted_count: i64 = project_invites
            .filter(project_id.eq(proj_id))
            .filter(invite_status.eq(InviteStatus::Accepted))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count expired invites
        let expired_count: i64 = project_invites
            .filter(project_id.eq(proj_id))
            .filter(invite_status.eq(InviteStatus::Expired))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count declined invites
        let declined_count: i64 = project_invites
            .filter(project_id.eq(proj_id))
            .filter(invite_status.eq(InviteStatus::Declined))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok((pending_count, accepted_count, declined_count, expired_count))
    }
}
