//! Workspace member repository for managing workspace membership.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    Account, NewWorkspaceMember, UpdateWorkspaceMember, Workspace, WorkspaceMember,
};
use crate::types::{
    Cursor, CursorPage, CursorPagination, MemberFilter, MemberSortBy, MemberSortField,
    OffsetPagination, SortOrder, WorkspaceRole,
};
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
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceMember>>> + Send;

    /// Updates a workspace member with partial changes.
    fn update_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateWorkspaceMember,
    ) -> impl Future<Output = PgResult<WorkspaceMember>> + Send;

    /// Permanently removes a member from a workspace.
    fn remove_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists members of a workspace with offset pagination.
    ///
    /// Supports filtering by role and 2FA status, and sorting by name or date.
    fn offset_list_workspace_members(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: MemberSortBy,
        filter: MemberFilter,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Lists members of a workspace with cursor pagination.
    fn cursor_list_workspace_members(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: MemberFilter,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceMember>>> + Send;

    /// Lists workspaces where a user is a member.
    ///
    /// Returns memberships ordered by creation date.
    fn list_account_workspaces(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Lists user workspaces with full workspace details via JOIN.
    fn list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<(Workspace, WorkspaceMember)>>> + Send;

    /// Lists user workspaces with full workspace details using cursor pagination.
    fn cursor_list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<(Workspace, WorkspaceMember)>>> + Send;

    /// Gets a user's role in a workspace for permission checking.
    ///
    /// Returns the role if the user is a member, None otherwise.
    fn check_account_role(
        &mut self,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceRole>>> + Send;

    /// Finds all members with a specific role.
    fn find_members_by_role(
        &mut self,
        workspace_id: Uuid,
        role: WorkspaceRole,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceMember>>> + Send;

    /// Checks if a user has any access to a workspace.
    fn check_workspace_access(
        &mut self,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Lists members of a workspace with account details using offset pagination.
    ///
    /// Returns members with their associated account information (email, display name).
    fn offset_list_workspace_members_with_accounts(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: MemberSortBy,
        filter: MemberFilter,
    ) -> impl Future<Output = PgResult<Vec<(WorkspaceMember, Account)>>> + Send;

    /// Lists members of a workspace with account details using cursor pagination.
    ///
    /// Returns members with their associated account information (email, display name).
    fn cursor_list_workspace_members_with_accounts(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: MemberFilter,
    ) -> impl Future<Output = PgResult<CursorPage<(WorkspaceMember, Account)>>> + Send;

    /// Finds a workspace member with account details.
    fn find_workspace_member_with_account(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<(WorkspaceMember, Account)>>> + Send;

    /// Finds a workspace member by their email address.
    ///
    /// Performs a JOIN with accounts to match by email.
    fn find_workspace_member_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> impl Future<Output = PgResult<Option<(WorkspaceMember, Account)>>> + Send;

    /// Checks if two accounts share at least one common workspace.
    ///
    /// Returns true if both accounts are members of at least one common workspace.
    /// This is an optimized query that stops at the first match.
    fn accounts_share_workspace(
        &mut self,
        account_id_a: Uuid,
        account_id_b: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;
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
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<Option<WorkspaceMember>> {
        use schema::workspace_members::{self, dsl};

        let member = workspace_members::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(member_account_id))
            .select(WorkspaceMember::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn update_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateWorkspaceMember,
    ) -> PgResult<WorkspaceMember> {
        use schema::workspace_members::{self, dsl};

        let member = diesel::update(workspace_members::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(member_account_id))
            .set(&changes)
            .returning(WorkspaceMember::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn remove_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<()> {
        use schema::workspace_members::{self, dsl};

        diesel::delete(workspace_members::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(member_account_id))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn offset_list_workspace_members(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: MemberSortBy,
        filter: MemberFilter,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::{accounts, workspace_members};

        // Build base query with JOIN for name sorting
        let mut query = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(workspace_members::workspace_id.eq(workspace_id))
            .into_boxed();

        // Apply role filter
        if let Some(role) = filter.role {
            query = query.filter(workspace_members::member_role.eq(role));
        }

        // Note: has_2fa filter is not yet implemented as accounts table
        // doesn't have a 2FA field. Will be added when 2FA is implemented.

        // Apply sorting
        let query = match (sort_by.field, sort_by.order) {
            (MemberSortField::Name, SortOrder::Asc) => query.order(accounts::display_name.asc()),
            (MemberSortField::Name, SortOrder::Desc) => query.order(accounts::display_name.desc()),
            (MemberSortField::Date, SortOrder::Asc) => {
                query.order(workspace_members::created_at.asc())
            }
            (MemberSortField::Date, SortOrder::Desc) => {
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

    async fn cursor_list_workspace_members(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: MemberFilter,
    ) -> PgResult<CursorPage<WorkspaceMember>> {
        use schema::workspace_members::{self, dsl};

        // Get total count only if requested
        let total = if pagination.include_count {
            let mut count_query = workspace_members::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .into_boxed();

            if let Some(role) = filter.role {
                count_query = count_query.filter(dsl::member_role.eq(role));
            }

            Some(
                count_query
                    .count()
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Build query with cursor
        let mut query = workspace_members::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .into_boxed();

        if let Some(role) = filter.role {
            query = query.filter(dsl::member_role.eq(role));
        }

        if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query = query.filter(
                dsl::created_at.lt(cursor_ts).or(dsl::created_at
                    .eq(cursor_ts)
                    .and(dsl::account_id.lt(cursor.id))),
            );
        }

        let fetch_limit = pagination.fetch_limit();
        let mut items: Vec<WorkspaceMember> = query
            .select(WorkspaceMember::as_select())
            .order((dsl::created_at.desc(), dsl::account_id.desc()))
            .limit(fetch_limit)
            .load(self)
            .await
            .map_err(PgError::from)?;

        let has_more = items.len() as i64 > pagination.limit;
        if has_more {
            items.pop();
        }

        let next_cursor = if has_more {
            items.last().map(|m| {
                Cursor {
                    timestamp: m.created_at.into(),
                    id: m.account_id,
                }
                .encode()
            })
        } else {
            None
        };

        Ok(CursorPage {
            items,
            total,
            next_cursor,
        })
    }

    async fn list_account_workspaces(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::{self, dsl};

        let memberships = workspace_members::table
            .filter(dsl::account_id.eq(account_id))
            .select(WorkspaceMember::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(memberships)
    }

    async fn list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<(Workspace, WorkspaceMember)>> {
        use schema::{workspace_members, workspaces};

        let results = workspace_members::table
            .inner_join(workspaces::table.on(workspaces::id.eq(workspace_members::workspace_id)))
            .filter(workspace_members::account_id.eq(account_id))
            .filter(workspaces::deleted_at.is_null())
            .select((Workspace::as_select(), WorkspaceMember::as_select()))
            .order(workspace_members::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load::<(Workspace, WorkspaceMember)>(self)
            .await
            .map_err(PgError::from)?;

        Ok(results)
    }

    async fn cursor_list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<(Workspace, WorkspaceMember)>> {
        use diesel::dsl::count_star;
        use schema::{workspace_members, workspaces};

        // Build base filter
        let base_filter = workspace_members::account_id
            .eq(account_id)
            .and(workspaces::deleted_at.is_null());

        // Get total count only if requested
        let total = if pagination.include_count {
            Some(
                workspace_members::table
                    .inner_join(
                        workspaces::table.on(workspaces::id.eq(workspace_members::workspace_id)),
                    )
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Build query
        let mut query = workspace_members::table
            .inner_join(workspaces::table.on(workspaces::id.eq(workspace_members::workspace_id)))
            .filter(base_filter)
            .into_boxed();

        // Apply cursor filter if present
        if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query = query.filter(
                workspace_members::created_at
                    .lt(cursor_ts)
                    .or(workspace_members::created_at
                        .eq(cursor_ts)
                        .and(workspace_members::workspace_id.lt(cursor.id))),
            );
        }

        let items = query
            .order((
                workspace_members::created_at.desc(),
                workspace_members::workspace_id.desc(),
            ))
            .limit(pagination.fetch_limit())
            .select((Workspace::as_select(), WorkspaceMember::as_select()))
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(CursorPage::new(items, total, pagination.limit, |(_, m)| {
            (m.created_at.into(), m.workspace_id)
        }))
    }

    async fn check_account_role(
        &mut self,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> PgResult<Option<WorkspaceRole>> {
        use schema::workspace_members::{self, dsl};

        let role = workspace_members::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(account_id))
            .select(dsl::member_role)
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(role)
    }

    async fn find_members_by_role(
        &mut self,
        workspace_id: Uuid,
        role: WorkspaceRole,
    ) -> PgResult<Vec<WorkspaceMember>> {
        use schema::workspace_members::{self, dsl};

        let members = workspace_members::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::member_role.eq(role))
            .select(WorkspaceMember::as_select())
            .order(dsl::created_at.asc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn check_workspace_access(
        &mut self,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> PgResult<bool> {
        use schema::workspace_members::{self, dsl};

        let is_member = workspace_members::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(account_id))
            .select(dsl::account_id)
            .first::<Uuid>(self)
            .await
            .optional()
            .map_err(PgError::from)?
            .is_some();

        Ok(is_member)
    }

    async fn offset_list_workspace_members_with_accounts(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
        sort_by: MemberSortBy,
        filter: MemberFilter,
    ) -> PgResult<Vec<(WorkspaceMember, Account)>> {
        use schema::{accounts, workspace_members};

        let mut query = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(workspace_members::workspace_id.eq(workspace_id))
            .filter(accounts::deleted_at.is_null())
            .into_boxed();

        if let Some(role) = filter.role {
            query = query.filter(workspace_members::member_role.eq(role));
        }

        let query = match (sort_by.field, sort_by.order) {
            (MemberSortField::Name, SortOrder::Asc) => query.order(accounts::display_name.asc()),
            (MemberSortField::Name, SortOrder::Desc) => query.order(accounts::display_name.desc()),
            (MemberSortField::Date, SortOrder::Asc) => {
                query.order(workspace_members::created_at.asc())
            }
            (MemberSortField::Date, SortOrder::Desc) => {
                query.order(workspace_members::created_at.desc())
            }
        };

        let results = query
            .select((WorkspaceMember::as_select(), Account::as_select()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(results)
    }

    async fn cursor_list_workspace_members_with_accounts(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: MemberFilter,
    ) -> PgResult<CursorPage<(WorkspaceMember, Account)>> {
        use diesel::dsl::count_star;
        use schema::{accounts, workspace_members};

        // Build base filter
        let base_filter = workspace_members::workspace_id
            .eq(workspace_id)
            .and(accounts::deleted_at.is_null());

        // Get total count only if requested
        let total = if pagination.include_count {
            let mut count_query = workspace_members::table
                .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
                .filter(base_filter)
                .into_boxed();

            if let Some(role) = filter.role {
                count_query = count_query.filter(workspace_members::member_role.eq(role));
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

        // Build query with optional role filter
        let mut query = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(base_filter)
            .into_boxed();

        if let Some(role) = filter.role {
            query = query.filter(workspace_members::member_role.eq(role));
        }

        // Apply cursor filter if present
        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query
                .filter(
                    workspace_members::created_at
                        .lt(cursor_ts)
                        .or(workspace_members::created_at
                            .eq(cursor_ts)
                            .and(workspace_members::account_id.lt(cursor.id))),
                )
                .order((
                    workspace_members::created_at.desc(),
                    workspace_members::account_id.desc(),
                ))
                .limit(pagination.fetch_limit())
                .select((WorkspaceMember::as_select(), Account::as_select()))
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .order((
                    workspace_members::created_at.desc(),
                    workspace_members::account_id.desc(),
                ))
                .limit(pagination.fetch_limit())
                .select((WorkspaceMember::as_select(), Account::as_select()))
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |(m, _)| {
            (m.created_at.into(), m.account_id)
        }))
    }

    async fn find_workspace_member_with_account(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<Option<(WorkspaceMember, Account)>> {
        use schema::{accounts, workspace_members};

        let result = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(workspace_members::workspace_id.eq(workspace_id))
            .filter(workspace_members::account_id.eq(member_account_id))
            .filter(accounts::deleted_at.is_null())
            .select((WorkspaceMember::as_select(), Account::as_select()))
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(result)
    }

    async fn find_workspace_member_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> PgResult<Option<(WorkspaceMember, Account)>> {
        use schema::{accounts, workspace_members};

        let result = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(workspace_members::workspace_id.eq(workspace_id))
            .filter(accounts::email_address.eq(email))
            .filter(accounts::deleted_at.is_null())
            .select((WorkspaceMember::as_select(), Account::as_select()))
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(result)
    }

    async fn accounts_share_workspace(
        &mut self,
        account_id_a: Uuid,
        account_id_b: Uuid,
    ) -> PgResult<bool> {
        use diesel::dsl::exists;
        use schema::workspace_members;

        // Self-check: an account always "shares" with itself
        if account_id_a == account_id_b {
            return Ok(true);
        }

        // Use EXISTS with a self-join to find any common workspace
        // This is optimized to stop at the first match
        let wm_a = workspace_members::table;
        let wm_b = diesel::alias!(workspace_members as wm_b);

        let shares = diesel::select(exists(
            wm_a.inner_join(
                wm_b.on(wm_b
                    .field(workspace_members::workspace_id)
                    .eq(workspace_members::workspace_id)),
            )
            .filter(workspace_members::account_id.eq(account_id_a))
            .filter(wm_b.field(workspace_members::account_id).eq(account_id_b)),
        ))
        .get_result::<bool>(self)
        .await
        .map_err(PgError::from)?;

        Ok(shares)
    }
}
