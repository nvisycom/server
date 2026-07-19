//! Workspace pipeline runs repository for managing pipeline execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspacePipelineRun, UpdateWorkspacePipelineRun, WorkspacePipelineRun};
use crate::types::{
    CursorPage, CursorPagination, OffsetPagination, PipelineRunStatus, Slug, Username,
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

    /// Finds a workspace pipeline run by its unique identifier.
    fn find_workspace_pipeline_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipelineRun>>> + Send;

    /// Finds a run by ID, scoped to a workspace via its owning pipeline.
    ///
    /// Runs carry no workspace column, so this joins through the pipeline and
    /// filters on its workspace. A run whose pipeline is in another workspace
    /// is not found.
    fn find_pipeline_run_in_workspace(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipelineRun>>> + Send;

    /// Finds a run by its opaque id, scoped to a workspace, with the triggering
    /// account's handle.
    ///
    /// The run is addressed by its own id (behind `/runs/{runId}`); scoping
    /// through the owning pipeline keeps it workspace-bounded.
    fn find_workspace_run_by_id(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<(WorkspacePipelineRun, Option<Username>)>>> + Send;

    /// Finds a run by its `(pipeline, idempotency key)` pair, for detect replay.
    fn find_pipeline_run_by_idempotency_key(
        &mut self,
        pipeline_id: Uuid,
        idempotency_key: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipelineRun>>> + Send;

    /// Lists all runs for a specific pipeline with offset pagination.
    fn offset_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePipelineRun>>> + Send;

    /// Lists all runs for a specific pipeline with cursor pagination, each
    /// paired with the handle of the account that triggered it, if any.
    fn cursor_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<(WorkspacePipelineRun, Option<Username>)>>> + Send;

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
    ) -> impl Future<Output = PgResult<CursorPage<(WorkspacePipelineRun, Slug, Option<Username>)>>> + Send;

    /// Lists active runs (queued or running) for a specific pipeline.
    fn list_active_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePipelineRun>>> + Send;

    /// Updates a workspace pipeline run with new data.
    fn update_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspacePipelineRun,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;

    /// Marks a run as started.
    fn start_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;

    /// Marks a run as completed successfully.
    fn complete_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;

    /// Marks a run as failed.
    fn fail_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;

    /// Marks a run as cancelled.
    fn cancel_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspacePipelineRun>> + Send;

    /// Counts runs for a pipeline by status.
    fn count_workspace_pipeline_runs_by_status(
        &mut self,
        pipeline_id: Uuid,
        status: PipelineRunStatus,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Gets the most recent run for a pipeline.
    fn find_latest_workspace_pipeline_run(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipelineRun>>> + Send;
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

    async fn find_workspace_pipeline_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<Option<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = workspace_pipeline_runs::table
            .filter(dsl::id.eq(run_id))
            .select(WorkspacePipelineRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_pipeline_run_in_workspace(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> PgResult<Option<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::workspace_pipelines::dsl as pipelines;

        let run = runs::workspace_pipeline_runs
            .inner_join(pipelines::workspace_pipelines)
            .filter(runs::id.eq(run_id))
            .filter(pipelines::workspace_id.eq(workspace_id))
            .select(WorkspacePipelineRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_workspace_run_by_id(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> PgResult<Option<(WorkspacePipelineRun, Option<Username>)>> {
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::{accounts, workspace_pipeline_runs, workspace_pipelines};

        // Runs carry no workspace column; scope through the owning pipeline so
        // the id resolves only within its workspace.
        let run = workspace_pipeline_runs::table
            .inner_join(workspace_pipelines::table)
            .left_join(accounts::table)
            .filter(runs::id.eq(run_id))
            .filter(workspace_pipelines::workspace_id.eq(workspace_id))
            .select((
                WorkspacePipelineRun::as_select(),
                accounts::username.nullable(),
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

    async fn offset_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let runs = workspace_pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .order(dsl::started_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspacePipelineRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn cursor_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> PgResult<CursorPage<(WorkspacePipelineRun, Option<Username>)>> {
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
            .left_join(accounts::table)
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.limit + 1;

        let items: Vec<(WorkspacePipelineRun, Option<Username>)> =
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
                        WorkspacePipelineRun::as_select(),
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
            |(r, _): &(WorkspacePipelineRun, Option<Username>)| (r.started_at.into(), r.id),
        ))
    }

    async fn cursor_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> PgResult<CursorPage<(WorkspacePipelineRun, Slug, Option<Username>)>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::workspace_pipelines::dsl as pipelines;

        // Runs have no workspace column; scope them through the owning pipeline.
        // The owning pipeline's slug and the triggering account's handle are
        // selected alongside each run so the cross-pipeline response can address
        // each run by `(pipeline, number)` and name its trigger.
        let scoped = || {
            let mut query = runs::workspace_pipeline_runs
                .inner_join(pipelines::workspace_pipelines)
                .left_join(accounts::accounts)
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

        let limit = pagination.limit + 1;
        let selection = (
            WorkspacePipelineRun::as_select(),
            pipelines::slug,
            accounts::username.nullable(),
        );

        let items: Vec<(WorkspacePipelineRun, Slug, Option<Username>)> =
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
            |(run, _, _): &(WorkspacePipelineRun, Slug, Option<Username>)| {
                (run.started_at.into(), run.id)
            },
        ))
    }

    async fn list_active_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
    ) -> PgResult<Vec<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let runs = workspace_pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .filter(
                dsl::status
                    .eq(PipelineRunStatus::Running)
                    .or(dsl::status.eq(PipelineRunStatus::Analyzed)),
            )
            .order(dsl::started_at.desc())
            .select(WorkspacePipelineRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
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

    async fn start_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<WorkspacePipelineRun> {
        use diesel::dsl::now;
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = diesel::update(workspace_pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Running),
                dsl::started_at.eq(now),
            ))
            .returning(WorkspacePipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn complete_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<WorkspacePipelineRun> {
        use diesel::dsl::now;
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = diesel::update(workspace_pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Completed),
                dsl::completed_at.eq(now),
            ))
            .returning(WorkspacePipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn fail_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<WorkspacePipelineRun> {
        use diesel::dsl::now;
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = diesel::update(workspace_pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Failed),
                dsl::completed_at.eq(now),
            ))
            .returning(WorkspacePipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn cancel_workspace_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> PgResult<WorkspacePipelineRun> {
        use diesel::dsl::now;
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = diesel::update(workspace_pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Cancelled),
                dsl::completed_at.eq(now),
            ))
            .returning(WorkspacePipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn count_workspace_pipeline_runs_by_status(
        &mut self,
        pipeline_id: Uuid,
        status: PipelineRunStatus,
    ) -> PgResult<i64> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let count = workspace_pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .filter(dsl::status.eq(status))
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn find_latest_workspace_pipeline_run(
        &mut self,
        pipeline_id: Uuid,
    ) -> PgResult<Option<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::{self, dsl};

        let run = workspace_pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .order(dsl::started_at.desc())
            .select(WorkspacePipelineRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }
}
