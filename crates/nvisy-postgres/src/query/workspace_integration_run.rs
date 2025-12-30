//! Workspace runs repository for managing integration run tracking operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewWorkspaceIntegrationRun, WorkspaceIntegrationRun, UpdateWorkspaceIntegrationRun};
use crate::types::IntegrationStatus;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace run database operations.
///
/// Handles run lifecycle management including creation, status tracking,
/// filtering by status and type, and performance metrics.
pub trait WorkspaceIntegrationRunRepository {
    /// Creates a new workspace run with the provided configuration.
    fn create_run(
        &mut self,
        new_run: NewWorkspaceIntegrationRun,
    ) -> impl Future<Output = PgResult<WorkspaceIntegrationRun>> + Send;

    /// Finds a run by its unique identifier.
    fn find_run_by_id(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceIntegrationRun>>> + Send;

    /// Finds all runs for a workspace ordered by creation time.
    fn find_runs_by_workspace(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Finds all runs for a specific integration.
    fn find_runs_by_integration(
        &mut self,
        integration_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Finds runs matching a specific status for a workspace.
    fn find_runs_by_status(
        &mut self,
        workspace_id: Uuid,
        status: IntegrationStatus,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Finds runs of a specific type for a workspace.
    fn find_runs_by_type(
        &mut self,
        workspace_id: Uuid,
        run_type: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Finds all runs triggered by a specific account.
    fn find_runs_by_account(
        &mut self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Finds all failed runs for a workspace.
    fn find_failed_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Finds all currently in-progress runs.
    ///
    /// Optionally filtered by workspace.
    fn find_in_progress_runs(
        &mut self,
        workspace_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Finds runs created within the last 7 days for a workspace.
    fn find_recent_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;

    /// Updates a run with new status, results, or metadata.
    fn update_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspaceIntegrationRun,
    ) -> impl Future<Output = PgResult<WorkspaceIntegrationRun>> + Send;

    /// Marks a run as started with current timestamp.
    fn mark_run_started(
        &mut self,
        run_id: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceIntegrationRun>> + Send;

    /// Marks a run as completed with final status and results.
    fn mark_run_completed(
        &mut self,
        run_id: Uuid,
        status: IntegrationStatus,
        result_summary: Option<String>,
    ) -> impl Future<Output = PgResult<WorkspaceIntegrationRun>> + Send;

    /// Marks a run as failed with error details.
    fn mark_run_failed(
        &mut self,
        run_id: Uuid,
        error_details: serde_json::Value,
    ) -> impl Future<Output = PgResult<WorkspaceIntegrationRun>> + Send;

    /// Finds runs exceeding a duration threshold in milliseconds.
    fn find_long_running_runs(
        &mut self,
        min_duration_ms: i32,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegrationRun>>> + Send;
}

impl WorkspaceIntegrationRunRepository for PgConnection {
    async fn create_run(&mut self, new_run: NewWorkspaceIntegrationRun) -> PgResult<WorkspaceIntegrationRun> {
        use schema::workspace_integration_runs;

        let run = diesel::insert_into(workspace_integration_runs::table)
            .values(&new_run)
            .returning(WorkspaceIntegrationRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_run_by_id(&mut self, run_id: Uuid) -> PgResult<Option<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let run = workspace_integration_runs::table
            .filter(dsl::id.eq(run_id))
            .select(WorkspaceIntegrationRun::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_runs_by_workspace(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_integration(
        &mut self,
        integration_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::integration_id.eq(integration_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_status(
        &mut self,
        workspace_id: Uuid,
        status: IntegrationStatus,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::run_status.eq(status))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_type(
        &mut self,
        workspace_id: Uuid,
        run_type: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::run_type.eq(run_type))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_runs_by_account(
        &mut self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::account_id.eq(account_id))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_failed_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::run_status.eq(IntegrationStatus::Failed))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_in_progress_runs(
        &mut self,
        workspace_id: Option<Uuid>,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let mut query = workspace_integration_runs::table
            .filter(dsl::started_at.is_not_null())
            .filter(dsl::completed_at.is_null())
            .into_boxed();

        if let Some(proj_id) = workspace_id {
            query = query.filter(dsl::workspace_id.eq(proj_id));
        }

        let runs = query
            .order(dsl::started_at.desc())
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn find_recent_runs(
        &mut self,
        workspace_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let seven_days_ago = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().days(7));

        let runs = workspace_integration_runs::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::created_at.gt(seven_days_ago))
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }

    async fn update_run(
        &mut self,
        run_id: Uuid,
        updates: UpdateWorkspaceIntegrationRun,
    ) -> PgResult<WorkspaceIntegrationRun> {
        use schema::workspace_integration_runs::{self, dsl};

        let run = diesel::update(workspace_integration_runs::table.filter(dsl::id.eq(run_id)))
            .set(&updates)
            .returning(WorkspaceIntegrationRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn mark_run_started(&mut self, run_id: Uuid) -> PgResult<WorkspaceIntegrationRun> {
        use schema::workspace_integration_runs::{self, dsl};

        let run = diesel::update(workspace_integration_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::started_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
                dsl::run_status.eq(IntegrationStatus::Executing),
            ))
            .returning(WorkspaceIntegrationRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn mark_run_completed(
        &mut self,
        run_id: Uuid,
        status: IntegrationStatus,
        result_summary: Option<String>,
    ) -> PgResult<WorkspaceIntegrationRun> {
        use schema::workspace_integration_runs::{self, dsl};

        let run = workspace_integration_runs::table
            .filter(dsl::id.eq(run_id))
            .select(WorkspaceIntegrationRun::as_select())
            .first::<WorkspaceIntegrationRun>(self)
            .await
            .map_err(PgError::from)?;

        let now = jiff::Timestamp::now();
        let duration_ms = run.started_at.map(|started| {
            let duration = now - jiff::Timestamp::from(started);
            duration.total(jiff::Unit::Millisecond).ok().unwrap_or(0.0) as i32
        });

        let run = diesel::update(workspace_integration_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::completed_at.eq(Some(jiff_diesel::Timestamp::from(now))),
                dsl::run_status.eq(status),
                dsl::duration_ms.eq(duration_ms),
                dsl::result_summary.eq(result_summary),
            ))
            .returning(WorkspaceIntegrationRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn mark_run_failed(
        &mut self,
        run_id: Uuid,
        error_details: serde_json::Value,
    ) -> PgResult<WorkspaceIntegrationRun> {
        use schema::workspace_integration_runs::{self, dsl};

        let run = workspace_integration_runs::table
            .filter(dsl::id.eq(run_id))
            .select(WorkspaceIntegrationRun::as_select())
            .first::<WorkspaceIntegrationRun>(self)
            .await
            .map_err(PgError::from)?;

        let now = jiff::Timestamp::now();
        let duration_ms = run.started_at.map(|started| {
            let duration = now - jiff::Timestamp::from(started);
            duration.total(jiff::Unit::Millisecond).ok().unwrap_or(0.0) as i32
        });

        let run = diesel::update(workspace_integration_runs::table.filter(dsl::id.eq(run_id)))
            .set((
                dsl::completed_at.eq(Some(jiff_diesel::Timestamp::from(now))),
                dsl::run_status.eq(IntegrationStatus::Failed),
                dsl::duration_ms.eq(duration_ms),
                dsl::error_details.eq(Some(error_details)),
            ))
            .returning(WorkspaceIntegrationRun::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(run)
    }

    async fn find_long_running_runs(
        &mut self,
        min_duration_ms: i32,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegrationRun>> {
        use schema::workspace_integration_runs::{self, dsl};

        let runs = workspace_integration_runs::table
            .filter(dsl::duration_ms.is_not_null())
            .filter(dsl::duration_ms.gt(min_duration_ms))
            .order(dsl::duration_ms.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceIntegrationRun::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(runs)
    }
}
