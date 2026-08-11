//! Workspace pipeline runs repository for managing pipeline execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspacePipelineRun, UpdateWorkspacePipelineRun, WorkspacePipeline, WorkspacePipelineRun,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, Handle, PipelineRunStatus, WithAccountRef,
};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace pipeline run database operations.
///
/// Handles pipeline run lifecycle management including creation, status updates,
/// completion tracking, and queries.
pub trait WorkspacePipelineRunRepository {
    /// Creates a new workspace pipeline run record.
    fn create_workspace_pipeline_run(
        &mut self,
        new_run: NewWorkspacePipelineRun,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;

    /// Finds a run by its opaque id, scoped to a workspace, returning the run
    /// and its owning pipeline.
    ///
    /// The run is addressed by its own id (behind `/runs/{runId}`); scoping
    /// through the owning pipeline keeps it workspace-bounded and hides runs of
    /// soft-deleted pipelines.
    fn find_workspace_run_by_id(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<(WorkspacePipelineRun, WorkspacePipeline)>>> + Send;

    /// Finds a run by its `(pipeline, idempotency key)` pair, for detect replay.
    fn find_pipeline_run_by_idempotency_key(
        &mut self,
        pipeline_id: Uuid,
        idempotency_key: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipelineRun>>> + Send;

    /// Lists all runs for a specific pipeline with cursor pagination, each
    /// paired with the account that triggered it.
    fn cursor_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<WithAccountRef<WorkspacePipelineRun>>>> + Send;

    /// Lists all runs across a workspace's pipelines with cursor pagination.
    ///
    /// Runs carry no workspace reference of their own, so this joins through the
    /// owning pipeline and filters on its workspace. An optional status filter
    /// narrows the result; use [`cursor_list_workspace_pipeline_runs`] for a
    /// single pipeline.
    ///
    /// [`cursor_list_workspace_pipeline_runs`]: Self::cursor_list_workspace_pipeline_runs
    fn cursor_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<(WithAccountRef<WorkspacePipelineRun>, Handle)>>> + Send;

    /// Atomically claims a run for detection, transitioning it to `Analyzing`.
    ///
    /// Succeeds (returning the claimed run) only when the run is still `Queued`,
    /// or is `Analyzing` but its previous claim has gone stale (`claimed_at`
    /// older than `stale_before` — a worker that died mid-analysis). A run
    /// already being analyzed under a fresh claim, or past the detect phase,
    /// yields `None` so a redelivered job skips it instead of analyzing twice.
    ///
    /// The claim stamps `claimed_at = now()`, so the lease renews on each
    /// (re)claim. Callers pass `stale_before = now - lease` computed against the
    /// same clock the DB uses closely enough for a lease measured in minutes.
    fn claim_run_for_detection(
        &mut self,
        run_id: Uuid,
        stale_before: jiff::Timestamp,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipelineRun>>> + Send;

    /// Updates a workspace pipeline run with new data.
    fn update_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspacePipelineRun,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;

    /// Clears any run references to `file_id`, nulling `audit_file_id` and
    /// `output_file_id` wherever they point at it. Used when a file is
    /// soft-deleted (e.g. by retention): the `ON DELETE SET NULL` FKs fire only
    /// on a hard delete, so runs would otherwise keep pointing at a tombstoned
    /// file. Returns the number of runs updated.
    fn clear_run_file_references(
        &mut self,
        file_id: Uuid,
    ) -> impl Future<Output = PgResult<usize>> + Send;
}

impl WorkspacePipelineRunRepository for PgConnection {
    async fn create_workspace_pipeline_run(
        &mut self,
        new_run: NewWorkspacePipelineRun,
    ) -> PgResult<WorkspacePipelineRun> {
        use schema::workspace_pipeline_runs;

        let run = diesel::insert_into(workspace_pipeline_runs::table)
            .values(&new_run)
            .returning(WorkspacePipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_workspace_run_by_id(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> PgResult<Option<(WorkspacePipelineRun, WorkspacePipeline)>> {
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::{workspace_pipeline_runs, workspace_pipelines};

        // Runs carry no workspace column; scope through the owning pipeline so
        // the id resolves only within its workspace, and only while that
        // pipeline is live (a soft-deleted pipeline hides its runs). The
        // pipeline is returned alongside so callers need no second lookup.
        let run = workspace_pipeline_runs::table
            .inner_join(workspace_pipelines::table)
            .filter(runs::id.eq(run_id))
            .filter(workspace_pipelines::workspace_id.eq(workspace_id))
            .filter(workspace_pipelines::deleted_at.is_null())
            .select((
                WorkspacePipelineRun::as_select(),
                WorkspacePipeline::as_select(),
            ))
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_pipeline_run_by_idempotency_key(
        &mut self,
        pipeline_id: Uuid,
        idempotency_key: &str,
    ) -> PgResult<Option<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = workspace_pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .filter(dsl::idempotency_key.eq(idempotency_key))
            .select(WorkspacePipelineRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn cursor_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> PgResult<CursorPage<WithAccountRef<WorkspacePipelineRun>>> {
        use schema::workspace_pipeline_runs::dsl;
        use schema::{accounts, workspace_pipeline_runs};

        // Build base query with filters
        let mut base_query = workspace_pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
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

        // Rebuild query for fetching items
        let mut query = workspace_pipeline_runs::table
            .inner_join(accounts::table)
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.fetch_limit();

        let rows: Vec<(WorkspacePipelineRun, AccountRefRow)> =
            if let Some(cursor) = &pagination.after {
                let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

                query
                    .filter(
                        dsl::started_at
                            .lt(&cursor_time)
                            .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                    )
                    .select((
                        WorkspacePipelineRun::as_select(),
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
                        WorkspacePipelineRun::as_select(),
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

        let items: Vec<WithAccountRef<WorkspacePipelineRun>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            (wc.item.started_at.into(), wc.item.id)
        }))
    }

    async fn cursor_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> PgResult<CursorPage<(WithAccountRef<WorkspacePipelineRun>, Handle)>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::workspace_pipelines::dsl as pipelines;

        // Runs have no workspace column; scope them through the owning pipeline.
        // The owning pipeline's slug and the triggering account are selected
        // alongside each run so the cross-pipeline response can name its pipeline
        // and trigger (the run is addressed by its own id).
        let scoped = || {
            let mut query = runs::workspace_pipeline_runs
                .inner_join(pipelines::workspace_pipelines)
                .inner_join(accounts::accounts)
                .filter(pipelines::workspace_id.eq(workspace_id))
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

        let limit = pagination.fetch_limit();
        let selection = (
            WorkspacePipelineRun::as_select(),
            pipelines::slug,
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        );

        let rows: Vec<(WorkspacePipelineRun, Handle, AccountRefRow)> =
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

        let items: Vec<(WithAccountRef<WorkspacePipelineRun>, Handle)> = rows
            .into_iter()
            .map(|(item, slug, account)| (WithAccountRef { item, account }, slug))
            .collect();

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |(wc, _): &(WithAccountRef<WorkspacePipelineRun>, Handle)| {
                (wc.item.started_at.into(), wc.item.id)
            },
        ))
    }

    async fn claim_run_for_detection(
        &mut self,
        run_id: Uuid,
        stale_before: jiff::Timestamp,
    ) -> PgResult<Option<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let stale_before = jiff_diesel::Timestamp::from(stale_before);

        // Claim only if still queued, or analyzing under a claim that has gone
        // stale (a dead worker). The WHERE clause makes the transition atomic:
        // two concurrent deliveries race on the same row and exactly one flips
        // it to `analyzing`; the loser matches no row and gets `None`.
        let claimed = diesel::update(
            workspace_pipeline_runs::table
                .filter(dsl::id.eq(run_id))
                .filter(
                    dsl::status.eq(PipelineRunStatus::Queued).or(dsl::status
                        .eq(PipelineRunStatus::Analyzing)
                        .and(dsl::claimed_at.lt(stale_before))),
                ),
        )
        .set((
            dsl::status.eq(PipelineRunStatus::Analyzing),
            dsl::claimed_at.eq(diesel::dsl::now),
        ))
        .returning(WorkspacePipelineRun::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(PgError::from)?;

        Ok(claimed)
    }

    async fn update_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspacePipelineRun,
    ) -> PgResult<WorkspacePipelineRun> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = diesel::update(workspace_pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set(&updates)
            .returning(WorkspacePipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn clear_run_file_references(&mut self, file_id: Uuid) -> PgResult<usize> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let audit_cleared =
            diesel::update(workspace_pipeline_runs::table.filter(dsl::audit_file_id.eq(file_id)))
                .set(dsl::audit_file_id.eq(None::<Uuid>))
                .execute(self)
                .await
                .map_err(PgError::from)?;

        let output_cleared =
            diesel::update(workspace_pipeline_runs::table.filter(dsl::output_file_id.eq(file_id)))
                .set(dsl::output_file_id.eq(None::<Uuid>))
                .execute(self)
                .await
                .map_err(PgError::from)?;

        Ok(audit_cleared + output_cleared)
    }
}
