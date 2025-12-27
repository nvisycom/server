//! Project runs repository for managing integration run tracking operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectRun, ProjectRun, UpdateProjectRun};
use crate::types::IntegrationStatus;
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for project run database operations.
///
/// Handles run lifecycle management including creation, status tracking,
/// filtering by status and type, and performance metrics.
pub trait ProjectRunRepository {
    /// Creates a new project run with the provided configuration.
    fn create_run(
        &self,
        new_run: NewProjectRun,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    /// Finds a run by its unique identifier.
    fn find_run_by_id(
        &self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectRun>>> + Send;

    /// Finds all runs for a project ordered by creation time.
    fn find_runs_by_project(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Finds all runs for a specific integration.
    fn find_runs_by_integration(
        &self,
        integration_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Finds runs matching a specific status for a project.
    fn find_runs_by_status(
        &self,
        project_id: Uuid,
        status: IntegrationStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Finds runs of a specific type for a project.
    fn find_runs_by_type(
        &self,
        project_id: Uuid,
        run_type: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Finds all runs triggered by a specific account.
    fn find_runs_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Finds all failed runs for a project.
    fn find_failed_runs(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Finds all currently in-progress runs.
    ///
    /// Optionally filtered by project.
    fn find_in_progress_runs(
        &self,
        project_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Finds runs created within the last 7 days for a project.
    fn find_recent_runs(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    /// Updates a run with new status, results, or metadata.
    fn update_run(
        &self,
        run_id: Uuid,
        updates: UpdateProjectRun,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    /// Marks a run as started with current timestamp.
    fn mark_run_started(&self, run_id: Uuid) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    /// Marks a run as completed with final status and results.
    fn mark_run_completed(
        &self,
        run_id: Uuid,
        status: IntegrationStatus,
        result_summary: Option<String>,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    /// Marks a run as failed with error details.
    fn mark_run_failed(
        &self,
        run_id: Uuid,
        error_details: serde_json::Value,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    /// Finds runs exceeding a duration threshold in milliseconds.
    fn find_long_running_runs(
        &self,
        min_duration_ms: i32,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;
}

impl ProjectRunRepository for PgClient {
    async fn create_run(&self, new_run: NewProjectRun) -> PgResult<ProjectRun> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs;

        let run = diesel::insert_into(project_runs::table)
            .values(&new_run)
            .returning(ProjectRun::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_run_by_id(&self, run_id: Uuid) -> PgResult<Option<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let run = project_runs::table
            .filter(dsl::id.eq(run_id))
            .select(ProjectRun::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_runs_by_project(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_integration(
        &self,
        integration_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::integration_id.eq(integration_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_status(
        &self,
        project_id: Uuid,
        status: IntegrationStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::run_status.eq(status))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_type(
        &self,
        project_id: Uuid,
        run_type: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::run_type.eq(run_type))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::account_id.eq(account_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_failed_runs(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::run_status.eq(IntegrationStatus::Failed))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_in_progress_runs(&self, project_id: Option<Uuid>) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let mut query = project_runs::table
            .filter(dsl::started_at.is_not_null())
            .filter(dsl::completed_at.is_null())
            .into_boxed();

        if let Some(proj_id) = project_id {
            query = query.filter(dsl::project_id.eq(proj_id));
        }

        let runs = query
            .order(dsl::started_at.desc())
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_recent_runs(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let seven_days_ago = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().days(7));

        let runs = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::created_at.gt(seven_days_ago))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn update_run(&self, run_id: Uuid, updates: UpdateProjectRun) -> PgResult<ProjectRun> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let run = diesel::update(project_runs::table.filter(dsl::id.eq(run_id)))
            .set(&updates)
            .returning(ProjectRun::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn mark_run_started(&self, run_id: Uuid) -> PgResult<ProjectRun> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let run = diesel::update(project_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::started_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
                dsl::run_status.eq(IntegrationStatus::Executing),
            ))
            .returning(ProjectRun::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn mark_run_completed(
        &self,
        run_id: Uuid,
        status: IntegrationStatus,
        result_summary: Option<String>,
    ) -> PgResult<ProjectRun> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let run = project_runs::table
            .filter(dsl::id.eq(run_id))
            .select(ProjectRun::as_select())
            .first::<ProjectRun>(&mut conn)
            .await
            .map_err(PgError::from)?;

        let now = jiff::Timestamp::now();
        let duration_ms = run.started_at.map(|started| {
            let duration = now - jiff::Timestamp::from(started);
            duration.total(jiff::Unit::Millisecond).ok().unwrap_or(0.0) as i32
        });

        let run = diesel::update(project_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::completed_at.eq(Some(jiff_diesel::Timestamp::from(now))),
                dsl::run_status.eq(status),
                dsl::duration_ms.eq(duration_ms),
                dsl::result_summary.eq(result_summary),
            ))
            .returning(ProjectRun::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn mark_run_failed(
        &self,
        run_id: Uuid,
        error_details: serde_json::Value,
    ) -> PgResult<ProjectRun> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let run = project_runs::table
            .filter(dsl::id.eq(run_id))
            .select(ProjectRun::as_select())
            .first::<ProjectRun>(&mut conn)
            .await
            .map_err(PgError::from)?;

        let now = jiff::Timestamp::now();
        let duration_ms = run.started_at.map(|started| {
            let duration = now - jiff::Timestamp::from(started);
            duration.total(jiff::Unit::Millisecond).ok().unwrap_or(0.0) as i32
        });

        let run = diesel::update(project_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::completed_at.eq(Some(jiff_diesel::Timestamp::from(now))),
                dsl::run_status.eq(IntegrationStatus::Failed),
                dsl::duration_ms.eq(duration_ms),
                dsl::error_details.eq(Some(error_details)),
            ))
            .returning(ProjectRun::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_long_running_runs(
        &self,
        min_duration_ms: i32,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::duration_ms.is_not_null())
            .filter(dsl::duration_ms.gt(min_duration_ms))
            .order(dsl::duration_ms.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }
}
