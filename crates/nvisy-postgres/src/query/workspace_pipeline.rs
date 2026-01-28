//! Pipelines repository for managing workflow definitions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use uuid::Uuid;

use crate::model::{NewWorkspacePipeline, UpdateWorkspacePipeline, WorkspacePipeline};
use crate::types::{CursorPage, CursorPagination, OffsetPagination, PipelineStatus};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for pipeline database operations.
///
/// Handles pipeline lifecycle management including creation, updates,
/// status transitions, and queries.
pub trait WorkspacePipelineRepository {
    /// Creates a new pipeline record.
    fn create_workspace_pipeline(
        &mut self,
        new_pipeline: NewWorkspacePipeline,
    ) -> impl Future<Output = PgResult<WorkspacePipeline>> + Send;

    /// Finds a pipeline by its unique identifier.
    fn find_workspace_pipeline_by_id(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipeline>>> + Send;

    /// Finds a pipeline by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_pipeline_in_workspace(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspacePipeline>>> + Send;

    /// Lists all pipelines in a workspace with offset pagination.
    fn offset_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePipeline>>> + Send;

    /// Lists all pipelines in a workspace with cursor pagination.
    fn cursor_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineStatus>,
        search_term: Option<&str>,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspacePipeline>>> + Send;

    /// Lists all pipelines created by an account with offset pagination.
    fn offset_list_account_pipelines(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePipeline>>> + Send;

    /// Lists enabled pipelines in a workspace.
    fn list_enabled_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePipeline>>> + Send;

    /// Updates a pipeline with new data.
    fn update_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
        updates: UpdateWorkspacePipeline,
    ) -> impl Future<Output = PgResult<WorkspacePipeline>> + Send;

    /// Soft deletes a pipeline by setting the deletion timestamp.
    fn delete_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Counts pipelines in a workspace by status.
    fn count_workspace_pipelines_by_status(
        &mut self,
        workspace_id: Uuid,
        status: PipelineStatus,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Searches pipelines by name using trigram similarity.
    fn search_pipelines_by_name(
        &mut self,
        workspace_id: Uuid,
        search_term: &str,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<WorkspacePipeline>>> + Send;
}

impl WorkspacePipelineRepository for PgConnection {
    async fn create_workspace_pipeline(
        &mut self,
        new_pipeline: NewWorkspacePipeline,
    ) -> PgResult<WorkspacePipeline> {
        use schema::workspace_pipelines;

        let pipeline = diesel::insert_into(workspace_pipelines::table)
            .values(&new_pipeline)
            .returning(WorkspacePipeline::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn find_workspace_pipeline_by_id(
        &mut self,
        pipeline_id: Uuid,
    ) -> PgResult<Option<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipeline = workspace_pipelines::table
            .filter(dsl::id.eq(pipeline_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspacePipeline::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn find_pipeline_in_workspace(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
    ) -> PgResult<Option<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipeline = workspace_pipelines::table
            .filter(dsl::id.eq(pipeline_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspacePipeline::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn offset_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipelines = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspacePipeline::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn cursor_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineStatus>,
        search_term: Option<&str>,
    ) -> PgResult<CursorPage<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        // Build base query with filters
        let mut base_query = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply status filter
        if let Some(status) = status_filter {
            base_query = base_query.filter(dsl::status.eq(status));
        }

        // Apply search filter
        if let Some(term) = search_term {
            base_query = base_query.filter(dsl::name.trgm_similar_to(term));
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
        let mut query = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        if let Some(term) = search_term {
            query = query.filter(dsl::name.trgm_similar_to(term));
        }

        let limit = pagination.limit + 1;

        let items: Vec<WorkspacePipeline> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(WorkspacePipeline::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(WorkspacePipeline::as_select())
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
            |p: &WorkspacePipeline| (p.created_at.into(), p.id),
        ))
    }

    async fn offset_list_account_pipelines(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipelines = workspace_pipelines::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspacePipeline::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn list_enabled_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
    ) -> PgResult<Vec<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipelines = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::status.eq(PipelineStatus::Enabled))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::name.asc())
            .select(WorkspacePipeline::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn update_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
        updates: UpdateWorkspacePipeline,
    ) -> PgResult<WorkspacePipeline> {
        use schema::workspace_pipelines::{self, dsl};

        let pipeline = diesel::update(workspace_pipelines::table.filter(dsl::id.eq(pipeline_id)))
            .set(&updates)
            .returning(WorkspacePipeline::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn delete_workspace_pipeline(&mut self, pipeline_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::workspace_pipelines::{self, dsl};

        diesel::update(workspace_pipelines::table.filter(dsl::id.eq(pipeline_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn count_workspace_pipelines_by_status(
        &mut self,
        workspace_id: Uuid,
        status: PipelineStatus,
    ) -> PgResult<i64> {
        use schema::workspace_pipelines::{self, dsl};

        let count = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn search_pipelines_by_name(
        &mut self,
        workspace_id: Uuid,
        search_term: &str,
        limit: i64,
    ) -> PgResult<Vec<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipelines = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::name.trgm_similar_to(search_term))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::name.asc())
            .limit(limit)
            .select(WorkspacePipeline::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }
}
