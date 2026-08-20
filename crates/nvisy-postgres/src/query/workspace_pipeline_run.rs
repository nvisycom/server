//! Workspace pipeline runs repository for managing pipeline execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspacePipelineRun, UpdateWorkspacePipelineRun, WorkspacePipeline, WorkspacePipelineRun,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, Handle, PipelineRunStatus, RunFilter,
};
use crate::{PgConnection, PgError, PgResult, schema};

/// Resolved display names of a run's input and output files.
///
/// Each is `None` when the run has no such file yet (no output before redaction)
/// or the file has been removed (e.g. by retention).
#[derive(Debug, Default, Clone)]
pub struct RunFiles {
    /// Display name of the input document the run analyzes.
    pub input: Option<String>,
    /// Display name of the redacted output, once the run has produced one.
    pub output: Option<String>,
}

/// One row of a pipeline-run listing: the run plus the context a response needs
/// to render it without follow-up lookups — the triggering account, the owning
/// pipeline's slug, and the input file's display name (`None` if the file was
/// removed).
#[derive(Debug, Clone)]
pub struct PipelineRunListRow {
    /// The run.
    pub run: WorkspacePipelineRun,
    /// The account that triggered the run.
    pub account: AccountRefRow,
    /// Slug of the run's owning pipeline.
    pub pipeline_slug: Handle,
    /// Display name of the run's input document, if still present.
    pub input_file_name: Option<String>,
}

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

    /// Lists a specific pipeline's runs with cursor pagination. `filter` narrows
    /// by status and/or file (its `pipeline_id` is ignored — the listing is
    /// already pipeline-scoped).
    fn cursor_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        filter: &RunFilter,
    ) -> impl Future<Output = PgResult<CursorPage<PipelineRunListRow>>> + Send;

    /// Lists all runs across a workspace's pipelines with cursor pagination.
    ///
    /// Runs carry no workspace reference of their own, so this joins through the
    /// owning pipeline and filters on its workspace. `filter` narrows by status,
    /// file, and/or owning pipeline; use [`cursor_list_workspace_pipeline_runs`]
    /// for a single pipeline.
    ///
    /// [`cursor_list_workspace_pipeline_runs`]: Self::cursor_list_workspace_pipeline_runs
    fn cursor_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: &RunFilter,
    ) -> impl Future<Output = PgResult<CursorPage<PipelineRunListRow>>> + Send;

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

    /// Resolves the display names of a run's input and output files.
    ///
    /// Two indexed lookups by id (the output only when the run has produced one);
    /// a file removed (e.g. by retention) yields `None` for that name. Used to
    /// name a single run's files in its response without threading a join
    /// through the shared run lookup.
    fn run_file_names(
        &mut self,
        workspace_id: Uuid,
        run: &WorkspacePipelineRun,
    ) -> impl Future<Output = PgResult<RunFiles>> + Send;

    /// Updates a workspace pipeline run with new data.
    fn update_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspacePipelineRun,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;
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
        filter: &RunFilter,
    ) -> PgResult<CursorPage<PipelineRunListRow>> {
        use schema::workspace_pipeline_runs::dsl;
        use schema::{accounts, workspace_files, workspace_pipeline_runs, workspace_pipelines};

        // Build base query with filters. The listing is already scoped to one
        // pipeline, so `filter.pipeline_id` is not applied here.
        let mut base_query = workspace_pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = filter.status {
            base_query = base_query.filter(dsl::status.eq(status));
        }
        if let Some(file_id) = filter.input_file_id {
            base_query = base_query.filter(dsl::input_file_id.eq(file_id));
        }
        if let Some(account_id) = filter.account_id {
            base_query = base_query.filter(dsl::account_id.eq(account_id));
        }
        if let Some(trigger_type) = filter.trigger_type {
            base_query = base_query.filter(dsl::trigger_type.eq(trigger_type));
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

        // Rebuild query for fetching items. Join the owning pipeline (for its
        // slug) and the input file (to name the run's analyzed document) so a
        // row is self-contained; a LEFT JOIN on the file tolerates one removed
        // by retention, yielding a null name.
        let mut query = workspace_pipeline_runs::table
            .inner_join(accounts::table)
            .inner_join(workspace_pipelines::table)
            .left_join(workspace_files::table.on(dsl::input_file_id.eq(workspace_files::id)))
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = filter.status {
            query = query.filter(dsl::status.eq(status));
        }
        if let Some(file_id) = filter.input_file_id {
            query = query.filter(dsl::input_file_id.eq(file_id));
        }
        if let Some(account_id) = filter.account_id {
            query = query.filter(dsl::account_id.eq(account_id));
        }
        if let Some(trigger_type) = filter.trigger_type {
            query = query.filter(dsl::trigger_type.eq(trigger_type));
        }

        let limit = pagination.fetch_limit();
        let selection = (
            WorkspacePipelineRun::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
            workspace_pipelines::slug,
            workspace_files::display_name.nullable(),
        );

        let rows: Vec<(WorkspacePipelineRun, AccountRefRow, Handle, Option<String>)> =
            if let Some(cursor) = &pagination.after {
                let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

                query
                    .filter(
                        dsl::started_at
                            .lt(&cursor_time)
                            .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                    )
                    .select(selection)
                    .order((dsl::started_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            } else {
                query
                    .select(selection)
                    .order((dsl::started_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            };

        let items = rows
            .into_iter()
            .map(
                |(run, account, pipeline_slug, input_file_name)| PipelineRunListRow {
                    run,
                    account,
                    pipeline_slug,
                    input_file_name,
                },
            )
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            (row.run.started_at.into(), row.run.id)
        }))
    }

    async fn cursor_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: &RunFilter,
    ) -> PgResult<CursorPage<PipelineRunListRow>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_files::dsl as files;
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::workspace_pipelines::dsl as pipelines;

        // Runs have no workspace column; scope them through the owning pipeline.
        // The owning pipeline's slug, the triggering account, and the input
        // file's name are selected alongside each run so the cross-pipeline
        // response can name its pipeline, trigger, and analyzed document without
        // a per-row lookup. The input file is LEFT-joined so a file removed by
        // retention yields a null name rather than dropping the run.
        let scoped = || {
            let mut query = runs::workspace_pipeline_runs
                .inner_join(pipelines::workspace_pipelines)
                .inner_join(accounts::accounts)
                .left_join(files::workspace_files.on(runs::input_file_id.eq(files::id)))
                .filter(pipelines::workspace_id.eq(workspace_id))
                .into_boxed();
            if let Some(status) = filter.status {
                query = query.filter(runs::status.eq(status));
            }
            if let Some(file_id) = filter.input_file_id {
                query = query.filter(runs::input_file_id.eq(file_id));
            }
            if let Some(pipeline_id) = filter.pipeline_id {
                query = query.filter(runs::pipeline_id.eq(pipeline_id));
            }
            if let Some(account_id) = filter.account_id {
                query = query.filter(runs::account_id.eq(account_id));
            }
            if let Some(trigger_type) = filter.trigger_type {
                query = query.filter(runs::trigger_type.eq(trigger_type));
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
            files::display_name.nullable(),
        );

        let rows: Vec<(WorkspacePipelineRun, Handle, AccountRefRow, Option<String>)> =
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

        let items = rows
            .into_iter()
            .map(
                |(run, pipeline_slug, account, input_file_name)| PipelineRunListRow {
                    run,
                    account,
                    pipeline_slug,
                    input_file_name,
                },
            )
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            (row.run.started_at.into(), row.run.id)
        }))
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

    async fn run_file_names(
        &mut self,
        workspace_id: Uuid,
        run: &WorkspacePipelineRun,
    ) -> PgResult<RunFiles> {
        use schema::workspace_files::{self, dsl};

        // Select the display name only, scoped to the workspace and excluding
        // soft-deleted files, so a file removed by retention resolves to `None`.
        async fn name_of(
            conn: &mut PgConnection,
            workspace_id: Uuid,
            file_id: Uuid,
        ) -> PgResult<Option<String>> {
            workspace_files::table
                .filter(dsl::id.eq(file_id))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .select(dsl::display_name)
                .first::<String>(conn)
                .await
                .optional()
                .map_err(PgError::from)
        }

        let input = name_of(self, workspace_id, run.input_file_id).await?;
        let output = match run.output_file_id {
            Some(output_file_id) => name_of(self, workspace_id, output_file_id).await?,
            None => None,
        };

        Ok(RunFiles { input, output })
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
}
