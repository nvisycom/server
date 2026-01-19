//! Pipeline runs repository for managing pipeline execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewPipelineRun, PipelineRun, UpdatePipelineRun};
use crate::types::{CursorPage, CursorPagination, OffsetPagination, PipelineRunStatus};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for pipeline run database operations.
///
/// Handles pipeline run lifecycle management including creation, status updates,
/// completion tracking, and queries.
pub trait PipelineRunRepository {
    /// Creates a new pipeline run record.
    fn create_pipeline_run(
        &mut self,
        new_run: NewPipelineRun,
    ) -> impl Future<Output = PgResult<PipelineRun>> + Send;

    /// Finds a pipeline run by its unique identifier.
    fn find_pipeline_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<PipelineRun>>> + Send;

    /// Finds a pipeline run by ID within a specific workspace.
    fn find_workspace_pipeline_run(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<PipelineRun>>> + Send;

    /// Lists all runs for a specific pipeline with offset pagination.
    fn offset_list_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<PipelineRun>>> + Send;

    /// Lists all runs for a specific pipeline with cursor pagination.
    fn cursor_list_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<PipelineRun>>> + Send;

    /// Lists all runs in a workspace with offset pagination.
    fn offset_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<PipelineRun>>> + Send;

    /// Lists all runs in a workspace with cursor pagination.
    fn cursor_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> impl Future<Output = PgResult<CursorPage<PipelineRun>>> + Send;

    /// Lists active runs (queued or running) in a workspace.
    fn list_active_workspace_runs(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<PipelineRun>>> + Send;

    /// Lists active runs (queued or running) for a specific pipeline.
    fn list_active_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<PipelineRun>>> + Send;

    /// Updates a pipeline run with new data.
    fn update_pipeline_run(
        &mut self,
        run_id: Uuid,
        updates: UpdatePipelineRun,
    ) -> impl Future<Output = PgResult<PipelineRun>> + Send;

    /// Marks a run as started.
    fn start_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<PipelineRun>> + Send;

    /// Marks a run as completed successfully.
    fn complete_pipeline_run(
        &mut self,
        run_id: Uuid,
        output_config: serde_json::Value,
        metrics: serde_json::Value,
    ) -> impl Future<Output = PgResult<PipelineRun>> + Send;

    /// Marks a run as failed with error details.
    fn fail_pipeline_run(
        &mut self,
        run_id: Uuid,
        error: serde_json::Value,
        metrics: serde_json::Value,
    ) -> impl Future<Output = PgResult<PipelineRun>> + Send;

    /// Marks a run as cancelled.
    fn cancel_pipeline_run(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<PipelineRun>> + Send;

    /// Counts runs for a pipeline by status.
    fn count_pipeline_runs_by_status(
        &mut self,
        pipeline_id: Uuid,
        status: PipelineRunStatus,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Gets the most recent run for a pipeline.
    fn find_latest_pipeline_run(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<PipelineRun>>> + Send;
}

impl PipelineRunRepository for PgConnection {
    async fn create_pipeline_run(&mut self, new_run: NewPipelineRun) -> PgResult<PipelineRun> {
        use schema::pipeline_runs;

        let run = diesel::insert_into(pipeline_runs::table)
            .values(&new_run)
            .returning(PipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_pipeline_run_by_id(&mut self, run_id: Uuid) -> PgResult<Option<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        let run = pipeline_runs::table
            .filter(dsl::id.eq(run_id))
            .select(PipelineRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_workspace_pipeline_run(
        &mut self,
        workspace_id: Uuid,
        run_id: Uuid,
    ) -> PgResult<Option<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        let run = pipeline_runs::table
            .filter(dsl::id.eq(run_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .select(PipelineRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn offset_list_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        let runs = pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(PipelineRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn cursor_list_pipeline_runs(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> PgResult<CursorPage<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        // Build base query with filters
        let mut base_query = pipeline_runs::table
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
        let mut query = pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.limit + 1;

        let items: Vec<PipelineRun> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(PipelineRun::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(PipelineRun::as_select())
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
            |r: &PipelineRun| (r.created_at.into(), r.id),
        ))
    }

    async fn offset_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        let runs = pipeline_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(PipelineRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn cursor_list_workspace_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineRunStatus>,
    ) -> PgResult<CursorPage<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        // Build base query with filters
        let mut base_query = pipeline_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
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
        let mut query = pipeline_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        let limit = pagination.limit + 1;

        let items: Vec<PipelineRun> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(PipelineRun::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(PipelineRun::as_select())
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
            |r: &PipelineRun| (r.created_at.into(), r.id),
        ))
    }

    async fn list_active_workspace_runs(
        &mut self,
        workspace_id: Uuid,
    ) -> PgResult<Vec<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        let runs = pipeline_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(
                dsl::status
                    .eq(PipelineRunStatus::Queued)
                    .or(dsl::status.eq(PipelineRunStatus::Running)),
            )
            .order(dsl::created_at.desc())
            .select(PipelineRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn list_active_pipeline_runs(&mut self, pipeline_id: Uuid) -> PgResult<Vec<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        let runs = pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .filter(
                dsl::status
                    .eq(PipelineRunStatus::Queued)
                    .or(dsl::status.eq(PipelineRunStatus::Running)),
            )
            .order(dsl::created_at.desc())
            .select(PipelineRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn update_pipeline_run(
        &mut self,
        run_id: Uuid,
        updates: UpdatePipelineRun,
    ) -> PgResult<PipelineRun> {
        use schema::pipeline_runs::{self, dsl};

        let run = diesel::update(pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set(&updates)
            .returning(PipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn start_pipeline_run(&mut self, run_id: Uuid) -> PgResult<PipelineRun> {
        use diesel::dsl::now;
        use schema::pipeline_runs::{self, dsl};

        let run = diesel::update(pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Running),
                dsl::started_at.eq(now),
            ))
            .returning(PipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn complete_pipeline_run(
        &mut self,
        run_id: Uuid,
        output_config: serde_json::Value,
        metrics: serde_json::Value,
    ) -> PgResult<PipelineRun> {
        use diesel::dsl::now;
        use schema::pipeline_runs::{self, dsl};

        let run = diesel::update(pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Completed),
                dsl::output_config.eq(output_config),
                dsl::metrics.eq(metrics),
                dsl::completed_at.eq(now),
            ))
            .returning(PipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn fail_pipeline_run(
        &mut self,
        run_id: Uuid,
        error: serde_json::Value,
        metrics: serde_json::Value,
    ) -> PgResult<PipelineRun> {
        use diesel::dsl::now;
        use schema::pipeline_runs::{self, dsl};

        let run = diesel::update(pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Failed),
                dsl::error.eq(Some(error)),
                dsl::metrics.eq(metrics),
                dsl::completed_at.eq(now),
            ))
            .returning(PipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn cancel_pipeline_run(&mut self, run_id: Uuid) -> PgResult<PipelineRun> {
        use diesel::dsl::now;
        use schema::pipeline_runs::{self, dsl};

        let run = diesel::update(pipeline_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::status.eq(PipelineRunStatus::Cancelled),
                dsl::completed_at.eq(now),
            ))
            .returning(PipelineRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn count_pipeline_runs_by_status(
        &mut self,
        pipeline_id: Uuid,
        status: PipelineRunStatus,
    ) -> PgResult<i64> {
        use schema::pipeline_runs::{self, dsl};

        let count = pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .filter(dsl::status.eq(status))
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn find_latest_pipeline_run(
        &mut self,
        pipeline_id: Uuid,
    ) -> PgResult<Option<PipelineRun>> {
        use schema::pipeline_runs::{self, dsl};

        let run = pipeline_runs::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .order(dsl::created_at.desc())
            .select(PipelineRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }
}
