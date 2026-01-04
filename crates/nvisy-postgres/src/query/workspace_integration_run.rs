//! Workspace integration runs repository for managing integration run tracking operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceIntegrationRun, UpdateWorkspaceIntegrationRun, WorkspaceIntegrationRun,
};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace integration run database operations.
///
/// Handles run lifecycle management including creation, status tracking,
/// and retrieval operations.
pub trait WorkspaceIntegrationRunRepository {
    /// Creates a new workspace integration run.
    fn create_workspace_integration_run(
        &mut self,
        new_run: NewWorkspaceIntegrationRun,
    ) -> impl Future<Output = PgResult<WorkspaceIntegrationRun>> + Send;

    /// Finds a workspace integration run by its unique identifier.
    fn find_workspace_integration_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceIntegrationRun>>> + Send;

    /// Updates a workspace integration run.
    fn update_workspace_integration_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspaceIntegrationRun,
    ) -> impl Future<Output = PgResult<WorkspaceIntegrationRun>> + Send;

    /// Lists workspace integration runs for a workspace with offset pagination.
    fn offset_list_workspace_integration_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Lists workspace integration runs for a workspace with cursor pagination.
    fn cursor_list_workspace_integration_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceIntegrationRun>>> + Send;

    /// Lists workspace integration runs for an integration with offset pagination.
    fn offset_list_integration_runs(
        &mut self,
        integration_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Lists workspace integration runs for an integration with cursor pagination.
    fn cursor_list_integration_runs(
        &mut self,
        integration_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceIntegrationRun>>> + Send;
}

impl WorkspaceIntegrationRunRepository for PgConnection {
    async fn create_workspace_integration_run(
        &mut self,
        new_run: NewWorkspaceIntegrationRun,
    ) -> PgResult<WorkspaceIntegrationRun> {
        use schema::workspace_integration_runs;

        let run = diesel::insert_into(workspace_integration_runs::table)
            .values(&new_run)
            .returning(WorkspaceIntegrationRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_workspace_integration_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<Option<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let run = workspace_integration_runs::table
            .filter(dsl::id.eq(run_id))
            .select(WorkspaceIntegrationRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn update_workspace_integration_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspaceIntegrationRun,
    ) -> PgResult<WorkspaceIntegrationRun> {
        use schema::workspace_integration_runs::{self, dsl};

        let run = diesel::update(workspace_integration_runs::table.filter(dsl::id.eq(run_id)))
            .set(&updates)
            .returning(WorkspaceIntegrationRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn offset_list_workspace_integration_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn cursor_list_workspace_integration_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceIntegrationRun>> {
        use diesel::dsl::count_star;
        use schema::workspace_integration_runs::{self, dsl};

        let base_filter = dsl::workspace_id.eq(workspace_id);

        let total = if pagination.include_count {
            Some(
                workspace_integration_runs::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            workspace_integration_runs::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(WorkspaceIntegrationRun::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            workspace_integration_runs::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(WorkspaceIntegrationRun::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |r| {
            (r.created_at.into(), r.id)
        }))
    }

    async fn offset_list_integration_runs(
        &mut self,
        integration_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::integration_id.eq(integration_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn cursor_list_integration_runs(
        &mut self,
        integration_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceIntegrationRun>> {
        use diesel::dsl::count_star;
        use schema::workspace_integration_runs::{self, dsl};

        let base_filter = dsl::integration_id.eq(integration_id);

        let total = if pagination.include_count {
            Some(
                workspace_integration_runs::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            workspace_integration_runs::table
                .filter(base_filter)
                .filter(
                    dsl::created_at
                        .lt(cursor_ts)
                        .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(WorkspaceIntegrationRun::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            workspace_integration_runs::table
                .filter(base_filter)
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(WorkspaceIntegrationRun::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |r| {
            (r.created_at.into(), r.id)
        }))
    }
}
