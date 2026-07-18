//! Workspace policies repository for managing redaction policy config.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspacePolicy, UpdateWorkspacePolicy, WorkspacePolicy};
use crate::types::{CursorPage, CursorPagination, OffsetPagination, Username};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace policy database operations.
pub trait WorkspacePolicyRepository {
    /// Creates a new workspace policy record.
    fn create_workspace_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
    ) -> impl Future<Output = PgResult<WorkspacePolicy>> + Send;

    /// Finds a policy by its unique identifier.
    fn find_workspace_policy_by_id(
        &mut self,
        policy_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspacePolicy>>> + Send;

    /// Finds a policy by ID within a specific workspace.
    fn find_policy_in_workspace(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspacePolicy>>> + Send;

    /// Finds a policy by slug within a specific workspace, with the handle of
    /// the account that created it.
    fn find_policy_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> impl Future<Output = PgResult<Option<(WorkspacePolicy, Username)>>> + Send;

    /// Lists all policies in a workspace with offset pagination.
    fn offset_list_workspace_policies(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePolicy>>> + Send;

    /// Lists all policies in a workspace with cursor pagination, each paired
    /// with the handle of the account that created it.
    fn cursor_list_workspace_policies(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<(WorkspacePolicy, Username)>>> + Send;

    /// Updates a policy with new data.
    fn update_workspace_policy(
        &mut self,
        policy_id: Uuid,
        updates: UpdateWorkspacePolicy,
    ) -> impl Future<Output = PgResult<WorkspacePolicy>> + Send;

    /// Soft deletes a policy by setting the deletion timestamp.
    fn delete_workspace_policy(
        &mut self,
        policy_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Counts policies in a workspace.
    fn count_workspace_policies(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl WorkspacePolicyRepository for PgConnection {
    async fn create_workspace_policy(
        &mut self,
        new_policy: NewWorkspacePolicy,
    ) -> PgResult<WorkspacePolicy> {
        use schema::workspace_policies;

        let policy = diesel::insert_into(workspace_policies::table)
            .values(&new_policy)
            .returning(WorkspacePolicy::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(policy)
    }

    async fn find_workspace_policy_by_id(
        &mut self,
        policy_id: Uuid,
    ) -> PgResult<Option<WorkspacePolicy>> {
        use schema::workspace_policies::{self, dsl};

        let policy = workspace_policies::table
            .filter(dsl::id.eq(policy_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspacePolicy::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(policy)
    }

    async fn find_policy_in_workspace(
        &mut self,
        workspace_id: Uuid,
        policy_id: Uuid,
    ) -> PgResult<Option<WorkspacePolicy>> {
        use schema::workspace_policies::{self, dsl};

        let policy = workspace_policies::table
            .filter(dsl::id.eq(policy_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspacePolicy::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(policy)
    }

    async fn find_policy_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> PgResult<Option<(WorkspacePolicy, Username)>> {
        use schema::workspace_policies::dsl;
        use schema::{accounts, workspace_policies};

        let policy = workspace_policies::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::slug.eq(slug))
            .filter(dsl::deleted_at.is_null())
            .select((WorkspacePolicy::as_select(), accounts::username))
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(policy)
    }

    async fn offset_list_workspace_policies(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspacePolicy>> {
        use schema::workspace_policies::{self, dsl};

        let policies = workspace_policies::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspacePolicy::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(policies)
    }

    async fn cursor_list_workspace_policies(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<(WorkspacePolicy, Username)>> {
        use schema::workspace_policies::dsl;
        use schema::{accounts, workspace_policies};

        let total = if pagination.include_count {
            Some(
                workspace_policies::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .filter(dsl::deleted_at.is_null())
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let query = workspace_policies::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        let limit = pagination.limit + 1;

        let items: Vec<(WorkspacePolicy, Username)> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select((WorkspacePolicy::as_select(), accounts::username))
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select((WorkspacePolicy::as_select(), accounts::username))
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |(p, _): &(WorkspacePolicy, Username)| (p.created_at.into(), p.id),
        ))
    }

    async fn update_workspace_policy(
        &mut self,
        policy_id: Uuid,
        updates: UpdateWorkspacePolicy,
    ) -> PgResult<WorkspacePolicy> {
        use schema::workspace_policies::{self, dsl};

        let policy = diesel::update(workspace_policies::table.filter(dsl::id.eq(policy_id)))
            .set(&updates)
            .returning(WorkspacePolicy::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(policy)
    }

    async fn delete_workspace_policy(&mut self, policy_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::workspace_policies::{self, dsl};

        diesel::update(workspace_policies::table.filter(dsl::id.eq(policy_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn count_workspace_policies(&mut self, workspace_id: Uuid) -> PgResult<i64> {
        use schema::workspace_policies::{self, dsl};

        let count = workspace_policies::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }
}
