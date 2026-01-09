//! Workspace invite repository for managing workspace invitation operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{NewWorkspaceInvite, UpdateWorkspaceInvite, WorkspaceInvite};
use crate::types::{
    CursorPage, CursorPagination, InviteFilter, InviteSortBy, InviteSortField, InviteStatus,
    OffsetPagination, SortOrder,
};
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

    /// Finds a workspace invitation by its unique token string.
    fn find_workspace_invite_by_token(
        &mut self,
        token: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspaceInvite>>> + Send;

    /// Finds a workspace invitation by its unique identifier.
    fn find_workspace_invite_by_id(
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
    fn accept_workspace_invite(
        &mut self,
        invite_id: Uuid,
        _acceptor_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Rejects or declines a workspace invitation.
    fn reject_workspace_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Cancels a workspace invitation before it can be used.
    fn cancel_workspace_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Lists workspace invitations with offset pagination.
    ///
    /// Supports filtering by role and sorting by email or date.
    fn offset_list_workspace_invites(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Lists workspace invitations with cursor pagination.
    fn cursor_list_workspace_invites(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceInvite>>> + Send;

    /// Performs system-wide cleanup of expired workspace invitations.
    fn cleanup_expired_workspace_invites(&mut self)
    -> impl Future<Output = PgResult<usize>> + Send;

    /// Finds workspace invitations filtered by their current status.
    fn find_workspace_invites_by_status(
        &mut self,
        status: InviteStatus,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceInvite>>> + Send;

    /// Revokes a workspace invitation through administrative action.
    fn revoke_workspace_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
        _reason: Option<String>,
    ) -> impl Future<Output = PgResult<WorkspaceInvite>> + Send;

    /// Finds a pending workspace invitation by workspace and email.
    fn find_pending_workspace_invite_by_email(
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

    async fn find_workspace_invite_by_token(
        &mut self,
        token: &str,
    ) -> PgResult<Option<WorkspaceInvite>> {
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

    async fn find_workspace_invite_by_id(
        &mut self,
        invite_id: Uuid,
    ) -> PgResult<Option<WorkspaceInvite>> {
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

    async fn accept_workspace_invite(
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

    async fn reject_workspace_invite(
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

    async fn cancel_workspace_invite(
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

    async fn offset_list_workspace_invites(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> PgResult<Vec<WorkspaceInvite>> {
        use schema::workspace_invites;

        let mut query = workspace_invites::table
            .filter(workspace_invites::workspace_id.eq(workspace_id))
            .filter(workspace_invites::invite_status.ne(InviteStatus::Canceled))
            .into_boxed();

        // Apply role filter
        if let Some(role) = filter.role {
            query = query.filter(workspace_invites::invited_role.eq(role));
        }

        // Apply sorting
        let query = match (sort_by.field, sort_by.order) {
            (InviteSortField::Email, SortOrder::Asc) => query
                .filter(workspace_invites::invitee_email.is_not_null())
                .order(workspace_invites::invitee_email.asc()),
            (InviteSortField::Email, SortOrder::Desc) => query
                .filter(workspace_invites::invitee_email.is_not_null())
                .order(workspace_invites::invitee_email.desc()),
            (InviteSortField::Date, SortOrder::Asc) => {
                query.order(workspace_invites::created_at.asc())
            }
            (InviteSortField::Date, SortOrder::Desc) => {
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

    async fn cursor_list_workspace_invites(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> PgResult<CursorPage<WorkspaceInvite>> {
        use diesel::dsl::count_star;
        use schema::workspace_invites::{self, dsl};

        let sort_by_email = matches!(sort_by.field, InviteSortField::Email);

        let base_filter = dsl::workspace_id
            .eq(workspace_id)
            .and(dsl::invite_status.ne(InviteStatus::Canceled));

        // Build filtered query
        let mut query = workspace_invites::table
            .filter(base_filter.clone())
            .into_boxed();

        if let Some(role) = filter.role {
            query = query.filter(dsl::invited_role.eq(role));
        }
        if sort_by_email {
            query = query.filter(dsl::invitee_email.is_not_null());
        }
        if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query = query.filter(
                dsl::created_at
                    .lt(cursor_ts)
                    .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
            );
        }

        // Get total count
        let total = if pagination.include_count {
            let mut count_query = workspace_invites::table.filter(base_filter).into_boxed();
            if let Some(role) = filter.role {
                count_query = count_query.filter(dsl::invited_role.eq(role));
            }
            if sort_by_email {
                count_query = count_query.filter(dsl::invitee_email.is_not_null());
            }
            Some(
                count_query
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Execute with sort
        let items = match (sort_by.field, sort_by.order) {
            (InviteSortField::Email, SortOrder::Asc) => {
                query.order((dsl::invitee_email.asc(), dsl::id.asc()))
            }
            (InviteSortField::Email, SortOrder::Desc) => {
                query.order((dsl::invitee_email.desc(), dsl::id.desc()))
            }
            (InviteSortField::Date, SortOrder::Asc) => {
                query.order((dsl::created_at.asc(), dsl::id.asc()))
            }
            (InviteSortField::Date, SortOrder::Desc) => {
                query.order((dsl::created_at.desc(), dsl::id.desc()))
            }
        }
        .select(WorkspaceInvite::as_select())
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(PgError::from)?;

        Ok(CursorPage::new(items, total, pagination.limit, |i| {
            (i.created_at.into(), i.id)
        }))
    }

    async fn cleanup_expired_workspace_invites(&mut self) -> PgResult<usize> {
        use diesel::dsl::now;
        use schema::workspace_invites::dsl::*;

        let updated_count = diesel::update(workspace_invites)
            .filter(expires_at.lt(now))
            .filter(invite_status.eq(InviteStatus::Pending))
            .set(invite_status.eq(InviteStatus::Expired))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(updated_count)
    }

    async fn find_workspace_invites_by_status(
        &mut self,
        status: InviteStatus,
        pagination: OffsetPagination,
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

    async fn revoke_workspace_invite(
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

    async fn find_pending_workspace_invite_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> PgResult<Option<WorkspaceInvite>> {
        use diesel::dsl::now;
        use schema::workspace_invites::{self, dsl};

        let invite = workspace_invites::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::invitee_email.eq(email))
            .filter(dsl::invite_status.eq(InviteStatus::Pending))
            .filter(dsl::expires_at.gt(now))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(invite)
    }
}
