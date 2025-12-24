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
    fn create_run(
        &self,
        new_run: NewProjectRun,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    fn find_run_by_id(
        &self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectRun>>> + Send;

    fn find_runs_by_project(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn find_runs_by_integration(
        &self,
        integration_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn find_runs_by_status(
        &self,
        project_id: Uuid,
        status: IntegrationStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn find_runs_by_type(
        &self,
        project_id: Uuid,
        run_type: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn find_runs_by_account(
        &self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn find_failed_runs(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn find_in_progress_runs(
        &self,
        project_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn find_recent_runs(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;

    fn update_run(
        &self,
        run_id: Uuid,
        updates: UpdateProjectRun,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    fn mark_run_started(&self, run_id: Uuid) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    fn mark_run_completed(
        &self,
        run_id: Uuid,
        status: IntegrationStatus,
        result_summary: Option<String>,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    fn mark_run_failed(
        &self,
        run_id: Uuid,
        error_details: serde_json::Value,
    ) -> impl Future<Output = PgResult<ProjectRun>> + Send;

    fn count_runs_by_project(&self, project_id: Uuid)
    -> impl Future<Output = PgResult<i64>> + Send;

    fn count_runs_by_status(
        &self,
        project_id: Uuid,
        status: IntegrationStatus,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    fn get_run_stats(
        &self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<(i64, i64, i64, i64)>> + Send;

    fn find_long_running_runs(
        &self,
        min_duration_ms: i32,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectRun>>> + Send;
}

impl ProjectRunRepository for PgClient {
    /// Creates a new project run with the provided configuration.
    ///
    /// Inserts a new run record into the database, enabling tracking of
    /// integration executions, manual runs, and scheduled operations.
    /// This supports comprehensive run history and performance monitoring.
    ///
    /// # Arguments
    ///
    /// * `new_run` - Data for the new run including project, integration, and configuration
    ///
    /// # Returns
    ///
    /// The newly created `ProjectRun` with generated ID and timestamps,
    /// or a database error if the operation fails.
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

    /// Finds a run by its unique identifier.
    ///
    /// Retrieves a specific run using its UUID for detailed status viewing,
    /// result analysis, and run management operations.
    ///
    /// # Arguments
    ///
    /// * `run_id` - UUID of the run to retrieve
    ///
    /// # Returns
    ///
    /// The matching `ProjectRun` if found, `None` if not found,
    /// or a database error if the query fails.
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

    /// Finds all runs associated with a specific project.
    ///
    /// Retrieves all run history for a project, enabling comprehensive
    /// activity tracking and run history browsing. Results are ordered
    /// by creation time to show most recent runs first.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project whose runs to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectRun` entries for the project, ordered by
    /// creation time (most recent first), or a database error if the query fails.
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

    /// Finds all runs for a specific integration.
    ///
    /// Retrieves run history for a particular integration, enabling
    /// integration-specific performance tracking and troubleshooting.
    ///
    /// # Arguments
    ///
    /// * `integration_id` - UUID of the integration whose runs to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectRun` entries for the integration, ordered by
    /// creation time (most recent first), or a database error if the query fails.
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

    /// Finds runs matching a specific status for a project.
    ///
    /// Retrieves runs filtered by their execution status (pending, executing,
    /// success, failure), enabling status-specific monitoring and alerting
    /// workflows.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project to filter runs for
    /// * `status` - Status to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of matching `ProjectRun` entries ordered by creation time
    /// (most recent first), or a database error if the query fails.
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

    /// Finds runs of a specific type for a project.
    ///
    /// Retrieves runs filtered by their type (manual, scheduled, triggered),
    /// enabling type-specific analysis and workflow tracking.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project to filter runs for
    /// * `run_type` - Type of runs to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of matching `ProjectRun` entries ordered by creation time
    /// (most recent first), or a database error if the query fails.
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

    /// Finds all runs triggered by a specific account.
    ///
    /// Retrieves run history for manual runs initiated by a user,
    /// enabling user activity tracking and personal run management.
    ///
    /// # Arguments
    ///
    /// * `account_id` - UUID of the account whose runs to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectRun` entries triggered by the account, ordered by
    /// creation time (most recent first), or a database error if the query fails.
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

    /// Finds all failed runs for a project.
    ///
    /// Retrieves runs that ended with failure status, enabling error
    /// analysis and troubleshooting workflows. This supports monitoring
    /// and alerting for integration health.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project to find failed runs for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of failed `ProjectRun` entries ordered by creation time
    /// (most recent first), or a database error if the query fails.
    async fn find_failed_runs(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectRun>> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let runs = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::run_status.eq(IntegrationStatus::Failure))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(ProjectRun::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    /// Finds all currently in-progress runs.
    ///
    /// Retrieves runs that have started but not yet completed, enabling
    /// real-time monitoring of active operations. Can optionally filter
    /// by project or return all in-progress runs across the system.
    ///
    /// # Arguments
    ///
    /// * `project_id` - Optional UUID of the project to filter by
    ///
    /// # Returns
    ///
    /// A vector of in-progress `ProjectRun` entries ordered by start time
    /// (most recent first), or a database error if the query fails.
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

    /// Finds recently created runs for a project.
    ///
    /// Retrieves runs created within the last seven days, providing
    /// visibility into recent activity and enabling trend analysis.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project to find recent runs for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of recently created `ProjectRun` entries ordered by
    /// creation time (most recent first), or a database error if the query fails.
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

    /// Updates a run with new status, results, or metadata.
    ///
    /// Applies partial updates to an existing run using the provided
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged.
    ///
    /// # Arguments
    ///
    /// * `run_id` - UUID of the run to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `ProjectRun` with new values and timestamp,
    /// or a database error if the operation fails.
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

    /// Marks a run as started by setting the start timestamp and status.
    ///
    /// Updates a run to indicate execution has begun, setting the started_at
    /// timestamp and updating status to executing. This enables tracking of
    /// run duration and execution state.
    ///
    /// # Arguments
    ///
    /// * `run_id` - UUID of the run to mark as started
    ///
    /// # Returns
    ///
    /// The updated `ProjectRun` with start timestamp,
    /// or a database error if the operation fails.
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

    /// Marks a run as completed with final status and results.
    ///
    /// Updates a run to indicate execution has finished, setting the
    /// completion timestamp, final status, duration, and optional result
    /// summary. This completes the run lifecycle tracking.
    ///
    /// # Arguments
    ///
    /// * `run_id` - UUID of the run to mark as completed
    /// * `status` - Final status (success, failure, etc.)
    /// * `result_summary` - Optional summary of run results
    ///
    /// # Returns
    ///
    /// The updated `ProjectRun` with completion details,
    /// or a database error if the operation fails.
    async fn mark_run_completed(
        &self,
        run_id: Uuid,
        status: IntegrationStatus,
        result_summary: Option<String>,
    ) -> PgResult<ProjectRun> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        // First get the run to calculate duration
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

    /// Marks a run as failed with error details.
    ///
    /// Updates a run to indicate execution failed, setting the completion
    /// timestamp, failure status, duration, and detailed error information
    /// for troubleshooting.
    ///
    /// # Arguments
    ///
    /// * `run_id` - UUID of the run to mark as failed
    /// * `error_details` - JSON object containing error details
    ///
    /// # Returns
    ///
    /// The updated `ProjectRun` with failure details,
    /// or a database error if the operation fails.
    async fn mark_run_failed(
        &self,
        run_id: Uuid,
        error_details: serde_json::Value,
    ) -> PgResult<ProjectRun> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        // First get the run to calculate duration
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
                dsl::run_status.eq(IntegrationStatus::Failure),
                dsl::duration_ms.eq(duration_ms),
                dsl::error_details.eq(Some(error_details)),
            ))
            .returning(ProjectRun::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    /// Counts total runs for a specific project.
    ///
    /// Calculates the total number of runs for a project, providing
    /// activity metrics for analytics and monitoring.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project to count runs for
    ///
    /// # Returns
    ///
    /// The total count of runs for the project,
    /// or a database error if the query fails.
    async fn count_runs_by_project(&self, project_id: Uuid) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let count = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Counts runs by status for a project.
    ///
    /// Calculates the number of runs in a specific status, enabling
    /// status distribution analysis and health monitoring.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project to count runs for
    /// * `status` - Status to filter by
    ///
    /// # Returns
    ///
    /// The count of runs with the specified status,
    /// or a database error if the query fails.
    async fn count_runs_by_status(
        &self,
        project_id: Uuid,
        status: IntegrationStatus,
    ) -> PgResult<i64> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let count = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::run_status.eq(status))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Gets comprehensive run statistics for a project.
    ///
    /// Calculates multiple run metrics in a single query: total runs,
    /// successful runs, failed runs, and in-progress runs. This provides
    /// a comprehensive overview of project run health.
    ///
    /// # Arguments
    ///
    /// * `project_id` - UUID of the project to get statistics for
    ///
    /// # Returns
    ///
    /// A tuple of (total, successful, failed, in_progress) counts,
    /// or a database error if the query fails.
    async fn get_run_stats(&self, project_id: Uuid) -> PgResult<(i64, i64, i64, i64)> {
        let mut conn = self.get_connection().await?;

        use schema::project_runs::{self, dsl};

        let total = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(PgError::from)?;

        // Note: IntegrationStatus doesn't have a Success variant
        // Counting completed runs that are not failures as "successful"
        let successful = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::completed_at.is_not_null())
            .filter(dsl::run_status.ne(IntegrationStatus::Failure))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(PgError::from)?;

        let failed = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::run_status.eq(IntegrationStatus::Failure))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(PgError::from)?;

        let in_progress = project_runs::table
            .filter(dsl::project_id.eq(project_id))
            .filter(dsl::started_at.is_not_null())
            .filter(dsl::completed_at.is_null())
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok((total, successful, failed, in_progress))
    }

    /// Finds runs that exceeded a specific duration threshold.
    ///
    /// Retrieves completed runs that took longer than the specified duration,
    /// enabling performance analysis and identification of slow operations.
    ///
    /// # Arguments
    ///
    /// * `min_duration_ms` - Minimum duration in milliseconds
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of long-running `ProjectRun` entries ordered by duration
    /// (longest first), or a database error if the query fails.
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
