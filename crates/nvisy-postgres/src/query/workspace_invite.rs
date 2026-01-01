//! Workspace invite repository for managing workspace invitation operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewWorkspaceInvite, UpdateWorkspaceInvite, WorkspaceInvite};
use crate::types::{InviteFilter, InviteSortBy, InviteStatus, SortOrder};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace invitation database operations.
///
/// Handles workspace invitations including creation, acceptance, rejection, and token
/// management with expiration tracking.
pub trait WorkspaceInviteRepository {
    /// Creates a new workspace invitation with secure token generation.
    fn create_workspace_invite(
        &mut self,
        invite: NewWorkspaceInvite,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Finds an invitation by its unique token string.
    fn find_invite_by_token(
        &mut self,
        token: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspaceInvite>>> + Send;

    /// Finds an invitation by its unique identifier.
    fn find_invite_by_id(
        &mut self,
        invite_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceInvite>>> + Send;

    /// Updates a workspace invitation with new values and status changes.
    fn update_workspace_invite(
        &mut self,
        invite_id: Uuid,
        changes: UpdateWorkspaceInvite,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Accepts a workspace invitation and marks it as successfully processed.
    fn accept_invite(
        &mut self,
        invite_id: Uuid,
        _acceptor_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Rejects or declines a workspace invitation.
    fn reject_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Cancels a workspace invitation before it can be used.
    fn cancel_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Lists all invitations for a specific workspace with pagination support.
    fn list_workspace_invites(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Lists invitations for a workspace with sorting and filtering options.
    ///
    /// Supports filtering by role and sorting by email or date.
    /// Note: Sorting by email requires a JOIN with accounts table.
    fn list_workspace_invites_filtered(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Lists invitations for a specific email with pagination support.
    fn list_invites_by_email(
        &mut self,
        email: Option<&str>,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Performs system-wide cleanup of expired invitations.
    fn cleanup_expired_invites(&mut self) -> impl Future<Output = PgResult<usize>> + Send;

    /// Retrieves all pending invitations for a specific workspace.
    fn get_pending_invites(
        &mut self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Finds invitations filtered by their current status.
    fn find_invites_by_status(
        &mut self,
        status: InviteStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Finds invitations that are approaching their expiration time.
    fn find_expiring_invites(
        &mut self,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Revokes an invitation through administrative action.
    fn revoke_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
        _reason: Option<String>,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Retrieves an invitation by its unique identifier.
    fn get_invite_by_id(
        &mut self,
        invite_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceInvite>>> + Send;

    /// Lists invitations for a workspace.
    fn list_workspace_invites_with_filter(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Finds a pending invite by workspace and email.
    fn find_pending_invite_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspaceInvite>>> + Send;
}

impl WorkspaceInviteRepository for PgConnection {
    async fn create_workspace_invite(
        &mut self,
        invite: NewWorkspaceInvite,
    ) -> PgResult<WorkspaceInvite> {
        use schema::workspace_invites;

        let invite = diesel::insert_into(workspace_invites::table)
            .values(&invite)
            .returning(WorkspaceInvite::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn find_invite_by_token(&mut self, token: &str) -> PgResult<Option<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invite = workspace_invites
            .filter(invite_token.eq(token))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn find_invite_by_id(&mut self, invite_id: Uuid) -> PgResult<Option<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invite = workspace_invites
            .filter(id.eq(invite_id))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn update_workspace_invite(
        &mut self,
        invite_id: Uuid,
        changes: UpdateWorkspaceInvite,
    ) -> PgResult<WorkspaceInvite> {
        use schema::workspace_invites::dsl::*;

        let invite = diesel::update(workspace_invites)
            .filter(id.eq(invite_id))
            .set(&changes)
            .returning(WorkspaceInvite::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn accept_invite(
        &mut self,
        invite_id: Uuid,
        _acceptor_id: Uuid,
    ) -> PgResult<WorkspaceInvite> {
        let changes = UpdateWorkspaceInvite {
            invite_status: Some(InviteStatus::Accepted),
            responded_at: Some(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
            ..Default::default()
        };

        self.update_workspace_invite(invite_id, changes).await
    }

    async fn reject_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> PgResult<WorkspaceInvite> {
        let changes = UpdateWorkspaceInvite {
            invite_status: Some(InviteStatus::Declined),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_workspace_invite(invite_id, changes).await
    }

    async fn cancel_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> PgResult<WorkspaceInvite> {
        let changes = UpdateWorkspaceInvite {
            invite_status: Some(InviteStatus::Canceled),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_workspace_invite(invite_id, changes).await
    }

    async fn list_workspace_invites(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invites = workspace_invites
            .filter(workspace_id.eq(proj_id))
            .select(WorkspaceInvite::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn list_workspace_invites_filtered(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> PgResult<Vec<WorkspaceInvite>> {
        use schema::workspace_invites;

        let mut query = workspace_invites::table
            .filter(workspace_invites::workspace_id.eq(proj_id))
            .into_boxed();

        // Apply role filter
        if let Some(role) = filter.role {
            query = query.filter(workspace_invites::invited_role.eq(role));
        }

        // Apply sorting
        let query = match sort_by {
            InviteSortBy::Email(SortOrder::Asc) => {
                query.order(workspace_invites::invitee_email.asc().nulls_last())
            }
            InviteSortBy::Email(SortOrder::Desc) => {
                query.order(workspace_invites::invitee_email.desc().nulls_last())
            }
            InviteSortBy::Date(SortOrder::Asc) => query.order(workspace_invites::created_at.asc()),
            InviteSortBy::Date(SortOrder::Desc) => {
                query.order(workspace_invites::created_at.desc())
            }
        };

        let invites = query
            .select(WorkspaceInvite::as_select())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn list_invites_by_email(
        &mut self,
        email: Option<&str>,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let mut query = workspace_invites.into_boxed();

        if let Some(e) = email {
            query = query.filter(invitee_email.eq(e));
        }

        let invites = query
            .select(WorkspaceInvite::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn cleanup_expired_invites(&mut self) -> PgResult<usize> {
        use schema::workspace_invites::dsl::*;

        let now = jiff_diesel::Timestamp::from(Timestamp::now());

        let updated_count = diesel::update(workspace_invites)
            .filter(expires_at.lt(now))
            .filter(invite_status.eq(InviteStatus::Pending))
            .set(invite_status.eq(InviteStatus::Expired))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(updated_count)
    }

    async fn get_pending_invites(&mut self, proj_id: Uuid) -> PgResult<Vec<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invites = workspace_invites
            .filter(workspace_id.eq(proj_id))
            .filter(invite_status.eq(InviteStatus::Pending))
            .filter(expires_at.gt(jiff_diesel::Timestamp::from(Timestamp::now())))
            .select(WorkspaceInvite::as_select())
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
    ) -> PgResult<Vec<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invites = workspace_invites
            .filter(invite_status.eq(status))
            .select(WorkspaceInvite::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(invites)
    }

    async fn find_expiring_invites(&mut self, hours: i64) -> PgResult<Vec<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let expiry_threshold =
            jiff_diesel::Timestamp::from(Timestamp::now() + Span::new().hours(hours));

        let invites = workspace_invites
            .filter(invite_status.eq(InviteStatus::Pending))
            .filter(expires_at.between(
                jiff_diesel::Timestamp::from(Timestamp::now()),
                expiry_threshold,
            ))
            .select(WorkspaceInvite::as_select())
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
    ) -> PgResult<WorkspaceInvite> {
        let changes = UpdateWorkspaceInvite {
            invite_status: Some(InviteStatus::Revoked),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_workspace_invite(invite_id, changes).await
    }

    async fn get_invite_by_id(&mut self, invite_id: Uuid) -> PgResult<Option<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invite = workspace_invites
            .filter(id.eq(invite_id))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }

    async fn list_workspace_invites_with_filter(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> PgResult<Vec<WorkspaceInvite>> {
        self.list_workspace_invites_filtered(proj_id, pagination, sort_by, filter)
            .await
    }

    async fn find_pending_invite_by_email(
        &mut self,
        ws_id: Uuid,
        email: &str,
    ) -> PgResult<Option<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invite = workspace_invites
            .filter(workspace_id.eq(ws_id))
            .filter(invitee_email.eq(email))
            .filter(invite_status.eq(InviteStatus::Pending))
            .filter(expires_at.gt(jiff_diesel::Timestamp::from(Timestamp::now())))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }
}
