//! Workspace connection runs repository for managing sync execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceConnectionRun, UpdateWorkspaceConnectionRun, WorkspaceConnectionRun,
};
use crate::types::{CursorPage, CursorPagination, SyncStatus, Username};
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

    /// Finds a run by ID, scoped to a workspace via its owning connection.
    ///
    /// Runs carry no workspace column, so this joins through the connection and
    /// filters on its workspace. A run whose connection is in another workspace
    /// is not found.
    fn find_connection_run_in_workspace(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnectionRun>>> + Send;

    /// Finds a sync run by its opaque id, with the triggering account's handle.
    fn find_connection_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<(WorkspaceConnectionRun, Option<Username>)>>> + Send;

    /// Lists runs for a specific connection with cursor pagination, each paired
    /// with the handle of the account that triggered it, if any.
    fn cursor_list_workspace_connection_runs(
        &mut self,
        connection_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<(WorkspaceConnectionRun, Option<Username>)>>> + Send;

    /// Lists all runs across a workspace's connections with cursor pagination.
    ///
    /// Runs carry no workspace reference of their own, so this joins through the
    /// owning connection and filters on its workspace. An optional status filter
    /// narrows the result; use [`cursor_list_workspace_connection_runs`] for a
    /// single connection.
    ///
    /// [`cursor_list_workspace_connection_runs`]: Self::cursor_list_workspace_connection_runs
    fn cursor_list_workspace_connection_runs_all(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<(WorkspaceConnectionRun, Uuid, Option<Username>)>>>
    + Send;

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

    async fn find_connection_run_in_workspace(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> PgResult<Option<WorkspaceConnectionRun>> {
        use schema::workspace_connection_runs::dsl as runs;
        use schema::workspace_connections::dsl as connections;

        let run = runs::workspace_connection_runs
            .inner_join(connections::workspace_connections)
            .filter(runs::id.eq(run_id))
            .filter(connections::workspace_id.eq(workspace_id))
            .select(WorkspaceConnectionRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_connection_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<Option<(WorkspaceConnectionRun, Option<Username>)>> {
        use schema::workspace_connection_runs::dsl;
        use schema::{accounts, workspace_connection_runs};

        let run = workspace_connection_runs::table
            .left_join(accounts::table)
            .filter(dsl::id.eq(run_id))
            .select((
                WorkspaceConnectionRun::as_select(),
                accounts::username.nullable(),
            ))
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
    ) -> PgResult<CursorPage<(WorkspaceConnectionRun, Option<Username>)>> {
        use schema::workspace_connection_runs::dsl;
        use schema::{accounts, workspace_connection_runs};

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
            .left_join(accounts::table)
            .filter(dsl::connection_id.eq(connection_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.limit + 1;

        let items: Vec<(WorkspaceConnectionRun, Option<Username>)> =
            if let Some(cursor) = &pagination.after {
                let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

                query
                    .filter(
                        dsl::started_at
                            .lt(&cursor_time)
                            .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                    )
                    .select((
                        WorkspaceConnectionRun::as_select(),
                        accounts::username.nullable(),
                    ))
                    .order((dsl::started_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            } else {
                query
                    .select((
                        WorkspaceConnectionRun::as_select(),
                        accounts::username.nullable(),
                    ))
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
            |(r, _): &(WorkspaceConnectionRun, Option<Username>)| (r.started_at.into(), r.id),
        ))
    }

    async fn cursor_list_workspace_connection_runs_all(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
    ) -> PgResult<CursorPage<(WorkspaceConnectionRun, Uuid, Option<Username>)>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_connection_runs::dsl as runs;
        use schema::workspace_connections::dsl as connections;

        // Runs have no workspace column; scope them through the owning
        // connection. The owning connection's slug and the triggering account's
        // handle are selected alongside each run so the cross-connection response
        // can address each run by `(connection, number)` and name its trigger.
        let scoped = || {
            let mut query = runs::workspace_connection_runs
                .inner_join(connections::workspace_connections)
                .left_join(accounts::accounts)
                .filter(connections::workspace_id.eq(workspace_id))
                .into_boxed();
            if let Some(status) = status_filter {
                query = query.filter(runs::status.eq(status));
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let limit = pagination.limit + 1;
        let selection = (
            WorkspaceConnectionRun::as_select(),
            connections::id,
            accounts::username.nullable(),
        );

        let items: Vec<(WorkspaceConnectionRun, Uuid, Option<Username>)> =
            if let Some(cursor) = &pagination.after {
                let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

                scoped()
                    .filter(
                        runs::started_at.lt(&cursor_time).or(runs::started_at
                            .eq(&cursor_time)
                            .and(runs::id.lt(cursor.id))),
                    )
                    .select(selection)
                    .order((runs::started_at.desc(), runs::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            } else {
                scoped()
                    .select(selection)
                    .order((runs::started_at.desc(), runs::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            };

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |(run, _, _): &(WorkspaceConnectionRun, Uuid, Option<Username>)| {
                (run.started_at.into(), run.id)
            },
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
