//! Workspace connection runs repository for managing sync execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceConnectionRun, UpdateWorkspaceConnectionRun, WorkspaceConnectionRun,
};
use crate::types::{CursorPage, CursorPagination, SyncStatus};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace connection run database operations.
///
/// Handles sync run lifecycle management including creation, status updates,
/// completion tracking, and queries.
pub trait WorkspaceConnectionRunRepository {
    /// Creates a new workspace connection run record.
    fn create_workspace_connection_run(
        &mut self,
        new_run: NewWorkspaceConnectionRun,
    ) -> impl Future<Output = PgResult<WorkspaceConnectionRun>> + Send;

    /// Finds a workspace connection run by its unique identifier.
    fn find_workspace_connection_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnectionRun>>> + Send;

    /// Lists runs for a specific connection with cursor pagination.
    fn cursor_list_workspace_connection_runs(
        &mut self,
        connection_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceConnectionRun>>> + Send;

    /// Gets the most recent run for a connection (its current sync state).
    fn find_latest_workspace_connection_run(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnectionRun>>> + Send;

    /// Updates a workspace connection run with new data.
    fn update_workspace_connection_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspaceConnectionRun,
    ) -> impl Future<Output = PgResult<WorkspaceConnectionRun>> + Send;

    /// Marks a run as completed successfully.
    fn complete_workspace_connection_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceConnectionRun>> + Send;

    /// Marks a run as failed, recording the error detail.
    fn fail_workspace_connection_run(
        &mut self,
        run_id: Uuid,
        error_message: &str,
    ) -> impl Future<Output = PgResult<WorkspaceConnectionRun>> + Send;

    /// Marks a run as cancelled.
    fn cancel_workspace_connection_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceConnectionRun>> + Send;
}

impl WorkspaceConnectionRunRepository for PgConnection {
    async fn create_workspace_connection_run(
        &mut self,
        new_run: NewWorkspaceConnectionRun,
    ) -> PgResult<WorkspaceConnectionRun> {
        use schema::workspace_connection_runs;

        let run = diesel::insert_into(workspace_connection_runs::table)
            .values(&new_run)
            .returning(WorkspaceConnectionRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_workspace_connection_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<Option<WorkspaceConnectionRun>> {
        use schema::workspace_connection_runs::{self, dsl};

        let run = workspace_connection_runs::table
            .filter(dsl::id.eq(run_id))
            .select(WorkspaceConnectionRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn cursor_list_workspace_connection_runs(
        &mut self,
        connection_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
    ) -> PgResult<CursorPage<WorkspaceConnectionRun>> {
        use schema::workspace_connection_runs::{self, dsl};

        let mut base_query = workspace_connection_runs::table
            .filter(dsl::connection_id.eq(connection_id))
            .into_boxed();

        if let Some(status) = status_filter {
            base_query = base_query.filter(dsl::status.eq(status));
        }

        let total = if pagination.include_count {
            Some(
                base_query
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let mut query = workspace_connection_runs::table
            .filter(dsl::connection_id.eq(connection_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.limit + 1;

        let items: Vec<WorkspaceConnectionRun> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::started_at
                        .lt(&cursor_time)
                        .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(WorkspaceConnectionRun::as_select())
                .order((dsl::started_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(WorkspaceConnectionRun::as_select())
                .order((dsl::started_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |r: &WorkspaceConnectionRun| (r.started_at.into(), r.id),
        ))
    }

    async fn find_latest_workspace_connection_run(
        &mut self,
        connection_id: Uuid,
    ) -> PgResult<Option<WorkspaceConnectionRun>> {
        use schema::workspace_connection_runs::{self, dsl};

        let run = workspace_connection_runs::table
            .filter(dsl::connection_id.eq(connection_id))
            .order(dsl::started_at.desc())
            .select(WorkspaceConnectionRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn update_workspace_connection_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspaceConnectionRun,
    ) -> PgResult<WorkspaceConnectionRun> {
        use schema::workspace_connection_runs::{self, dsl};

        let run = diesel::update(workspace_connection_runs::table.filter(dsl::id.eq(run_id)))
            .set(&updates)
            .returning(WorkspaceConnectionRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn complete_workspace_connection_run(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<WorkspaceConnectionRun> {
        use diesel::dsl::now;
        use schema::workspace_connection_runs::{self, dsl};

        let run = diesel::update(workspace_connection_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(SyncStatus::Completed),
                dsl::completed_at.eq(now),
            ))
            .returning(WorkspaceConnectionRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn fail_workspace_connection_run(
        &mut self,
        run_id: Uuid,
        error_message: &str,
    ) -> PgResult<WorkspaceConnectionRun> {
        use diesel::dsl::now;
        use schema::workspace_connection_runs::{self, dsl};

        let run = diesel::update(workspace_connection_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(SyncStatus::Failed),
                dsl::error_message.eq(error_message),
                dsl::completed_at.eq(now),
            ))
            .returning(WorkspaceConnectionRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn cancel_workspace_connection_run(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<WorkspaceConnectionRun> {
        use diesel::dsl::now;
        use schema::workspace_connection_runs::{self, dsl};

        let run = diesel::update(workspace_connection_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(SyncStatus::Cancelled),
                dsl::completed_at.eq(now),
            ))
            .returning(WorkspaceConnectionRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }
}
