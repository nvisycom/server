//! Workspace member repository for managing workspace membership.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewWorkspaceMember, UpdateWorkspaceMember, Workspace, WorkspaceMember};
use crate::types::{MemberFilter, MemberSortBy, SortOrder, WorkspaceRole};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace member database operations.
///
/// Handles workspace membership management including CRUD operations, role-based
/// access control, and activity tracking.
pub trait WorkspaceMemberRepository {
    /// Adds a new member to a workspace.
    fn add_workspace_member(
        &mut self,
        member: NewWorkspaceMember,
    ) -> impl Future<Output = PgResult<WorkspaceMember>> + Send;

    /// Finds a workspace member by workspace and account IDs.
    fn find_workspace_member(
        &mut self,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceMember>>> + Send;

    /// Updates a workspace member with partial changes.
    fn update_workspace_member(
        &mut self,
        proj_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateWorkspaceMember,
    ) -> impl Future<Output = PgResult<WorkspaceMember>> + Send;

    /// Permanently removes a member from a workspace.
    fn remove_workspace_member(
        &mut self,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists members of a workspace.
    ///
    /// Returns members ordered by role and creation date.
    fn list_workspace_members(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Lists members of a workspace with sorting and filtering options.
    ///
    /// Supports filtering by role and 2FA status, and sorting by name or date.
    /// Note: Sorting by name requires a JOIN with accounts table.
    fn list_workspace_members_filtered(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
        sort_by: MemberSortBy,
        filter: MemberFilter,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Lists workspaces where a user is a member.
    ///
    /// Returns memberships ordered by favorites and recent activity.
    fn list_user_workspaces(
        &mut self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Lists user workspaces with full workspace details via JOIN.
    fn list_user_workspaces_with_details(
        &mut self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<(Workspace, WorkspaceMember)>>> + Send;

    /// Gets a user's role in a workspace for permission checking.
    ///
    /// Returns the role if the user is a member, None otherwise.
    fn check_user_role(
        &mut self,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceRole>>> + Send;

    /// Updates the last access timestamp for a member.
    fn touch_member_access(
        &mut self,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Finds all members with a specific role.
    fn find_members_by_role(
        &mut self,
        proj_id: Uuid,
        role: WorkspaceRole,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Checks if a user has any access to a workspace.
    fn check_workspace_access(
        &mut self,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Finds members who have favorited the workspace.
    fn get_favorite_members(
        &mut self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Finds members who have enabled a specific notification type.
    fn get_notifiable_members(
        &mut self,
        proj_id: Uuid,
        notification_type: &str,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Finds members who accessed the workspace within the specified hours.
    fn get_recently_active_members(
        &mut self,
        proj_id: Uuid,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;
}

impl WorkspaceMemberRepository for PgConnection {
    async fn add_workspace_member(
        &mut self,
        member: NewWorkspaceMember,
    ) -> PgResult<WorkspaceMember> {
        use schema::workspace_members;

        let member = diesel::insert_into(workspace_members::table)
            .values(&member)
            .returning(WorkspaceMember::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn find_workspace_member(
        &mut self,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<Option<WorkspaceMember>> {
        use schema::workspace_members::dsl::*;

        let member = workspace_members
            .filter(workspace_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .select(WorkspaceMember::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn update_workspace_member(
        &mut self,
        proj_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateWorkspaceMember,
    ) -> PgResult<WorkspaceMember> {
        use schema::workspace_members::dsl::*;

        let member = diesel::update(workspace_members)
            .filter(workspace_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .set(&changes)
            .returning(WorkspaceMember::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn remove_workspace_member(
        &mut self,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<()> {
        use schema::workspace_members::dsl::*;

        diesel::delete(workspace_members)
            .filter(workspace_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_workspace_members(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::dsl::*;

        let members = workspace_members
            .filter(workspace_id.eq(proj_id))
            .select(WorkspaceMember::as_select())
            .order((member_role.asc(), created_at.asc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn list_workspace_members_filtered(
        &mut self,
        proj_id: Uuid,
        pagination: Pagination,
        sort_by: MemberSortBy,
        filter: MemberFilter,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::{accounts, workspace_members};

        // Build base query with JOIN for name sorting
        let mut query = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(workspace_members::workspace_id.eq(proj_id))
            .into_boxed();

        // Apply role filter
        if let Some(role) = filter.role {
            query = query.filter(workspace_members::member_role.eq(role));
        }

        // Note: has_2fa filter is not yet implemented as accounts table
        // doesn't have a 2FA field. Will be added when 2FA is implemented.

        // Apply sorting
        let query = match sort_by {
            MemberSortBy::Name(SortOrder::Asc) => query.order(accounts::display_name.asc()),
            MemberSortBy::Name(SortOrder::Desc) => query.order(accounts::display_name.desc()),
            MemberSortBy::Date(SortOrder::Asc) => query.order(workspace_members::created_at.asc()),
            MemberSortBy::Date(SortOrder::Desc) => {
                query.order(workspace_members::created_at.desc())
            }
        };

        let members = query
            .select(WorkspaceMember::as_select())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn list_user_workspaces(
        &mut self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::dsl::*;

        let memberships = workspace_members
            .filter(account_id.eq(user_id))
            .select(WorkspaceMember::as_select())
            .order((
                is_favorite.desc(),
                last_accessed_at.desc().nulls_last(),
                created_at.desc(),
            ))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(memberships)
    }

    async fn list_user_workspaces_with_details(
        &mut self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<(Workspace, WorkspaceMember)>> {
        use schema::{workspace_members, workspaces};

        let results = workspace_members::table
            .inner_join(workspaces::table.on(workspaces::id.eq(workspace_members::workspace_id)))
            .filter(workspace_members::account_id.eq(user_id))
            .filter(workspaces::deleted_at.is_null())
            .select((Workspace::as_select(), WorkspaceMember::as_select()))
            .order((
                workspace_members::is_favorite.desc(),
                workspace_members::last_accessed_at.desc().nulls_last(),
                workspace_members::created_at.desc(),
            ))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load::<(Workspace, WorkspaceMember)>(self)
            .await
            .map_err(PgError::from)?;

        Ok(results)
    }

    async fn check_user_role(
        &mut self,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> PgResult<Option<WorkspaceRole>> {
        use schema::workspace_members::dsl::*;

        let role = workspace_members
            .filter(workspace_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .select(member_role)
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(role)
    }

    async fn touch_member_access(&mut self, proj_id: Uuid, user_id: Uuid) -> PgResult<()> {
        use schema::workspace_members::dsl::*;

        diesel::update(workspace_members)
            .filter(workspace_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .set(last_accessed_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn find_members_by_role(
        &mut self,
        proj_id: Uuid,
        role: WorkspaceRole,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::dsl::*;

        let members = workspace_members
            .filter(workspace_id.eq(proj_id))
            .filter(member_role.eq(role))
            .select(WorkspaceMember::as_select())
            .order(created_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn check_workspace_access(&mut self, proj_id: Uuid, user_id: Uuid) -> PgResult<bool> {
        use schema::workspace_members::dsl::*;

        let is_member = workspace_members
            .filter(workspace_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .select(account_id)
            .first::<Uuid>(self)
            .await
            .optional()
            .map_err(PgError::from)?
            .is_some();

        Ok(is_member)
    }

    async fn get_favorite_members(&mut self, proj_id: Uuid) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::dsl::*;

        let members = workspace_members
            .filter(workspace_id.eq(proj_id))
            .filter(is_favorite.eq(true))
            .select(WorkspaceMember::as_select())
            .order(created_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn get_notifiable_members(
        &mut self,
        proj_id: Uuid,
        notification_type: &str,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::dsl::*;

        let mut query = workspace_members
            .filter(workspace_id.eq(proj_id))
            .into_boxed();

        match notification_type {
            "updates" => query = query.filter(notify_updates.eq(true)),
            "comments" => query = query.filter(notify_comments.eq(true)),
            "mentions" => query = query.filter(notify_mentions.eq(true)),
            _ => {}
        }

        let members = query
            .select(WorkspaceMember::as_select())
            .order(created_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn get_recently_active_members(
        &mut self,
        proj_id: Uuid,
        hours: i64,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::dsl::*;

        let cutoff_time = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(hours));

        let members = workspace_members
            .filter(workspace_id.eq(proj_id))
            .filter(last_accessed_at.gt(cutoff_time))
            .select(WorkspaceMember::as_select())
            .order(last_accessed_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }
}
