//! Workspace pipeline runs repository for managing pipeline execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspacePipelineRun, UpdateWorkspacePipelineRun, WorkspacePipelineRun};
use crate::types::{CursorPage, CursorPagination, OffsetPagination, PipelineRunStatus};
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

    /// Lists all runs for a specific pipeline with offset pagination.
    fn offset_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePipelineRun>>> + Send;

    /// Lists all runs for a specific pipeline with cursor pagination.
    fn cursor_list_workspace_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspacePipelineRun>>> + Send;

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
    ) -> PgResult<CursorPage<WorkspacePipelineRun>> {
        use schema::workspace_pipeline_runs::{self, dsl};

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
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.limit + 1;

        let items: Vec<WorkspacePipelineRun> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::started_at
                        .lt(&cursor_time)
                        .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(WorkspacePipelineRun::as_select())
                .order((dsl::started_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(WorkspacePipelineRun::as_select())
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
            |r: &WorkspacePipelineRun| (r.started_at.into(), r.id),
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
                    .eq(PipelineRunStatus::Queued)
                    .or(dsl::status.eq(PipelineRunStatus::Running)),
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
