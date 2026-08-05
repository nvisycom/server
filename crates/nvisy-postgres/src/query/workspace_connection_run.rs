//! Workspace connection runs repository for managing sync execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceConnectionRun, UpdateWorkspaceConnectionRun, WorkspaceConnectionRun,
};
use crate::types::{AccountRefRow, CursorPage, CursorPagination, SyncStatus, WithAccountRef};
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

    /// Returns the most recent successful sync completion time for each of the
    /// given connections.
    ///
    /// A connection's "last synced" instant is the `completed_at` of its latest
    /// run with status `Completed`; connections that have never synced
    /// successfully are absent from the result. This is a single grouped query
    /// so a page of connections costs one round-trip, not one per connection.
    fn last_successful_sync_at(
        &mut self,
        connection_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<Vec<(Uuid, jiff_diesel::Timestamp)>>> + Send;

    /// Lists runs for a specific connection with cursor pagination, each paired
    /// with the account that triggered it.
    fn cursor_list_workspace_connection_runs(
        &mut self,
        connection_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<WithAccountRef<WorkspaceConnectionRun>>>> + Send;

    /// Lists all runs across a workspace's connections with cursor pagination.
    ///
    /// Runs carry no workspace reference of their own, so this joins through the
    /// owning connection and filters on its workspace. An optional status filter
    /// and a set of providers narrow the result; an empty `providers` slice means
    /// no provider filter. Use [`cursor_list_workspace_connection_runs`] for a
    /// single connection.
    ///
    /// [`cursor_list_workspace_connection_runs`]: Self::cursor_list_workspace_connection_runs
    fn cursor_list_workspace_connection_runs_all(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
        providers: &[String],
    ) -> impl Future<Output = PgResult<CursorPage<(WithAccountRef<WorkspaceConnectionRun>, Uuid)>>> + Send;

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

    /// Marks a run as completed successfully with its final record count, only if
    /// it is still active.
    ///
    /// Returns the updated run, or `None` if the run was already in a terminal
    /// state (e.g. cancelled or reaped) and was therefore left unchanged. The
    /// count is written under the same status guard so a terminal run's fields
    /// are never mutated after the fact.
    fn complete_workspace_connection_run(
        &mut self,
        run_id: Uuid,
        records_synced: i64,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnectionRun>>> + Send;

    /// Marks a run as failed, recording the error detail, only if it is still
    /// active. Returns `None` if the run was already terminal.
    fn fail_workspace_connection_run(
        &mut self,
        run_id: Uuid,
        error_message: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnectionRun>>> + Send;

    /// Marks a run as cancelled, only if it is still active. Returns `None` if
    /// the run was already terminal and was therefore left unchanged.
    fn cancel_workspace_connection_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnectionRun>>> + Send;

    /// Fails all `Running` runs that started before `cutoff`, returning the
    /// number reaped. Recovers runs orphaned by a crash mid-sync.
    fn fail_stale_running_runs(
        &mut self,
        cutoff: jiff_diesel::Timestamp,
    ) -> impl Future<Output = PgResult<usize>> + Send;
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

    async fn last_successful_sync_at(
        &mut self,
        connection_ids: &[Uuid],
    ) -> PgResult<Vec<(Uuid, jiff_diesel::Timestamp)>> {
        use diesel::dsl::max;
        use schema::workspace_connection_runs::{self, dsl};

        if connection_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Only successful runs count toward "last synced"; a failed or cancelled
        // run does not move the timestamp. completed_at is non-null for any run
        // in a terminal state, so the grouped MAX is present for every group.
        workspace_connection_runs::table
            .filter(dsl::connection_id.eq_any(connection_ids))
            .filter(dsl::status.eq(SyncStatus::Completed))
            .group_by(dsl::connection_id)
            .select((dsl::connection_id, max(dsl::completed_at).assume_not_null()))
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn cursor_list_workspace_connection_runs(
        &mut self,
        connection_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
    ) -> PgResult<CursorPage<WithAccountRef<WorkspaceConnectionRun>>> {
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
            .inner_join(accounts::table)
            .filter(dsl::connection_id.eq(connection_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.fetch_limit();

        let rows: Vec<(WorkspaceConnectionRun, AccountRefRow)> =
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
                        (
                            accounts::username,
                            accounts::display_name,
                            accounts::avatar_url,
                        ),
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
                        (
                            accounts::username,
                            accounts::display_name,
                            accounts::avatar_url,
                        ),
                    ))
                    .order((dsl::started_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            };

        let items: Vec<WithAccountRef<WorkspaceConnectionRun>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            (wc.item.started_at.into(), wc.item.id)
        }))
    }

    async fn cursor_list_workspace_connection_runs_all(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<SyncStatus>,
        providers: &[String],
    ) -> PgResult<CursorPage<(WithAccountRef<WorkspaceConnectionRun>, Uuid)>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_connection_runs::dsl as runs;
        use schema::workspace_connections::dsl as connections;

        // Runs have no workspace column; scope them through the owning
        // connection. The owning connection's id and the triggering account are
        // selected alongside each run so the cross-connection response can name
        // its connection and trigger (the run is addressed by its own id).
        let scoped = || {
            let mut query = runs::workspace_connection_runs
                .inner_join(connections::workspace_connections)
                .inner_join(accounts::accounts)
                .filter(connections::workspace_id.eq(workspace_id))
                .into_boxed();
            if let Some(status) = status_filter {
                query = query.filter(runs::status.eq(status));
            }
            if !providers.is_empty() {
                query = query.filter(connections::provider.eq_any(providers.to_vec()));
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

        let limit = pagination.fetch_limit();
        let selection = (
            WorkspaceConnectionRun::as_select(),
            connections::id,
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        );

        let rows: Vec<(WorkspaceConnectionRun, Uuid, AccountRefRow)> =
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

        let items: Vec<(WithAccountRef<WorkspaceConnectionRun>, Uuid)> = rows
            .into_iter()
            .map(|(item, connection_id, account)| (WithAccountRef { item, account }, connection_id))
            .collect();

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |(wc, _): &(WithAccountRef<WorkspaceConnectionRun>, Uuid)| {
                (wc.item.started_at.into(), wc.item.id)
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
        records_synced: i64,
    ) -> PgResult<Option<WorkspaceConnectionRun>> {
        use diesel::dsl::now;
        use schema::workspace_connection_runs::{self, dsl};

        // Only transition from an active state, so a run already cancelled/reaped
        // is not resurrected as completed and its record count is not rewritten.
        let run = diesel::update(
            workspace_connection_runs::table
                .filter(dsl::id.eq(run_id))
                .filter(dsl::status.eq_any([SyncStatus::Pending, SyncStatus::Running])),
        )
        .set((
            dsl::status.eq(SyncStatus::Completed),
            dsl::records_synced.eq(records_synced),
            dsl::completed_at.eq(now),
        ))
        .returning(WorkspaceConnectionRun::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(PgError::from)?;

        Ok(run)
    }

    async fn fail_workspace_connection_run(
        &mut self,
        run_id: Uuid,
        error_message: &str,
    ) -> PgResult<Option<WorkspaceConnectionRun>> {
        use diesel::dsl::now;
        use schema::workspace_connection_runs::{self, dsl};

        // Only transition from an active state, so a terminal run is not
        // overwritten.
        let run = diesel::update(
            workspace_connection_runs::table
                .filter(dsl::id.eq(run_id))
                .filter(dsl::status.eq_any([SyncStatus::Pending, SyncStatus::Running])),
        )
        .set((
            dsl::status.eq(SyncStatus::Failed),
            dsl::error_message.eq(error_message),
            dsl::completed_at.eq(now),
        ))
        .returning(WorkspaceConnectionRun::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(PgError::from)?;

        Ok(run)
    }

    async fn cancel_workspace_connection_run(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<Option<WorkspaceConnectionRun>> {
        use diesel::dsl::now;
        use schema::workspace_connection_runs::{self, dsl};

        // Only transition from an active state, so a run that already completed,
        // failed, or was reaped is not overwritten as cancelled.
        let run = diesel::update(
            workspace_connection_runs::table
                .filter(dsl::id.eq(run_id))
                .filter(dsl::status.eq_any([SyncStatus::Pending, SyncStatus::Running])),
        )
        .set((
            dsl::status.eq(SyncStatus::Cancelled),
            dsl::completed_at.eq(now),
        ))
        .returning(WorkspaceConnectionRun::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(PgError::from)?;

        Ok(run)
    }

    async fn fail_stale_running_runs(&mut self, cutoff: jiff_diesel::Timestamp) -> PgResult<usize> {
        use diesel::dsl::now;
        use schema::workspace_connection_runs::{self, dsl};

        let reaped = diesel::update(
            workspace_connection_runs::table
                .filter(dsl::status.eq(SyncStatus::Running))
                .filter(dsl::started_at.lt(cutoff)),
        )
        .set((
            dsl::status.eq(SyncStatus::Failed),
            dsl::error_message.eq("Sync did not complete (reaped as stale)"),
            dsl::completed_at.eq(now),
        ))
        .execute(self)
        .await
        .map_err(PgError::from)?;

        Ok(reaped)
    }
}
