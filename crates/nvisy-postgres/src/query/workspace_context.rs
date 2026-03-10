//! Workspace contexts repository for managing context file metadata.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceContext, UpdateWorkspaceContext, WorkspaceContext};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace context database operations.
pub trait WorkspaceContextRepository {
    /// Creates a new workspace context record.
    fn create_workspace_context(
        &mut self,
        new_context: NewWorkspaceContext,
    ) -> impl Future<Output = PgResult<WorkspaceContext>> + Send;

    /// Finds a context by its unique identifier.
    fn find_workspace_context_by_id(
        &mut self,
        context_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceContext>>> + Send;

    /// Finds a context by ID within a specific workspace.
    fn find_context_in_workspace(
        &mut self,
        workspace_id: Uuid,
        context_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceContext>>> + Send;

    /// Lists all contexts in a workspace with offset pagination.
    fn offset_list_workspace_contexts(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceContext>>> + Send;

    /// Lists all contexts in a workspace with cursor pagination.
    fn cursor_list_workspace_contexts(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceContext>>> + Send;

    /// Updates a context with new data.
    fn update_workspace_context(
        &mut self,
        context_id: Uuid,
        updates: UpdateWorkspaceContext,
    ) -> impl Future<Output = PgResult<WorkspaceContext>> + Send;

    /// Soft deletes a context by setting the deletion timestamp.
    fn delete_workspace_context(
        &mut self,
        context_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Counts contexts in a workspace.
    fn count_workspace_contexts(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl WorkspaceContextRepository for PgConnection {
    async fn create_workspace_context(
        &mut self,
        new_context: NewWorkspaceContext,
    ) -> PgResult<WorkspaceContext> {
        use schema::workspace_contexts;

        let context = diesel::insert_into(workspace_contexts::table)
            .values(&new_context)
            .returning(WorkspaceContext::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(context)
    }

    async fn find_workspace_context_by_id(
        &mut self,
        context_id: Uuid,
    ) -> PgResult<Option<WorkspaceContext>> {
        use schema::workspace_contexts::{self, dsl};

        let context = workspace_contexts::table
            .filter(dsl::id.eq(context_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceContext::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(context)
    }

    async fn find_context_in_workspace(
        &mut self,
        workspace_id: Uuid,
        context_id: Uuid,
    ) -> PgResult<Option<WorkspaceContext>> {
        use schema::workspace_contexts::{self, dsl};

        let context = workspace_contexts::table
            .filter(dsl::id.eq(context_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceContext::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(context)
    }

    async fn offset_list_workspace_contexts(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceContext>> {
        use schema::workspace_contexts::{self, dsl};

        let contexts = workspace_contexts::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceContext::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(contexts)
    }

    async fn cursor_list_workspace_contexts(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceContext>> {
        use schema::workspace_contexts::{self, dsl};

        let total = if pagination.include_count {
            Some(
                workspace_contexts::table
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

        let query = workspace_contexts::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        let limit = pagination.limit + 1;

        let items: Vec<WorkspaceContext> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(WorkspaceContext::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(WorkspaceContext::as_select())
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
            |c: &WorkspaceContext| (c.created_at.into(), c.id),
        ))
    }

    async fn update_workspace_context(
        &mut self,
        context_id: Uuid,
        updates: UpdateWorkspaceContext,
    ) -> PgResult<WorkspaceContext> {
        use schema::workspace_contexts::{self, dsl};

        let context = diesel::update(workspace_contexts::table.filter(dsl::id.eq(context_id)))
            .set(&updates)
            .returning(WorkspaceContext::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(context)
    }

    async fn delete_workspace_context(&mut self, context_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::workspace_contexts::{self, dsl};

        diesel::update(workspace_contexts::table.filter(dsl::id.eq(context_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn count_workspace_contexts(&mut self, workspace_id: Uuid) -> PgResult<i64> {
        use schema::workspace_contexts::{self, dsl};

        let count = workspace_contexts::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }
}
