//! Pipelines repository for managing workflow definitions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use uuid::Uuid;

use crate::model::{NewWorkspacePipeline, UpdateWorkspacePipeline, WorkspacePipeline};
use crate::query::search::ilike_contains;
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, OffsetPagination, PipelineStatus, WithAccountRef,
};
use crate::{Error, PgConnection, Result, schema};

/// Repository for pipeline database operations.
///
/// Handles pipeline lifecycle management including creation, updates,
/// status transitions, and queries.
pub trait WorkspacePipelineRepository {
    /// Creates a new pipeline record.
    fn create_workspace_pipeline(
        &mut self,
        new_pipeline: NewWorkspacePipeline,
    ) -> impl Future<Output = Result<WorkspacePipeline>> + Send;

    /// Finds a pipeline by its unique identifier.
    fn find_workspace_pipeline_by_id(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspacePipeline>>> + Send;

    /// Finds a pipeline by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_pipeline_in_workspace(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspacePipeline>>> + Send;

    /// Finds a pipeline by slug within a specific workspace, with the handle and
    /// avatar of the account that created it.
    ///
    /// Excludes soft-deleted pipelines.
    fn find_pipeline_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspacePipeline>>>> + Send;

    /// Lists all pipelines in a workspace with offset pagination.
    fn offset_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<WorkspacePipeline>>> + Send;

    /// Lists all pipelines in a workspace with cursor pagination, each paired
    /// with the handle and avatar of the account that created it.
    fn cursor_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineStatus>,
        search_term: Option<&str>,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspacePipeline>>>> + Send;

    /// Lists all pipelines created by an account with offset pagination.
    fn offset_list_account_pipelines(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<WorkspacePipeline>>> + Send;

    /// Lists enabled pipelines in a workspace.
    fn list_enabled_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = Result<Vec<WorkspacePipeline>>> + Send;

    /// Updates a pipeline with new data.
    fn update_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
        updates: UpdateWorkspacePipeline,
    ) -> impl Future<Output = Result<WorkspacePipeline>> + Send;

    /// Soft deletes a pipeline by setting the deletion timestamp.
    fn delete_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Counts pipelines in a workspace by status.
    fn count_workspace_pipelines_by_status(
        &mut self,
        workspace_id: Uuid,
        status: PipelineStatus,
    ) -> impl Future<Output = Result<i64>> + Send;

    /// Searches pipelines by name: a case-insensitive substring match or a
    /// trigram-similarity match.
    fn search_pipelines_by_name(
        &mut self,
        workspace_id: Uuid,
        search_term: &str,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<WorkspacePipeline>>> + Send;
}

impl WorkspacePipelineRepository for PgConnection {
    async fn create_workspace_pipeline(
        &mut self,
        new_pipeline: NewWorkspacePipeline,
    ) -> Result<WorkspacePipeline> {
        use schema::workspace_pipelines;

        let pipeline = diesel::insert_into(workspace_pipelines::table)
            .values(&new_pipeline)
            .returning(WorkspacePipeline::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(pipeline)
    }

    async fn find_workspace_pipeline_by_id(
        &mut self,
        pipeline_id: Uuid,
    ) -> Result<Option<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipeline = workspace_pipelines::table
            .filter(dsl::id.eq(pipeline_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspacePipeline::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(pipeline)
    }

    async fn find_pipeline_in_workspace(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Uuid,
    ) -> Result<Option<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipeline = workspace_pipelines::table
            .filter(dsl::id.eq(pipeline_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspacePipeline::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(pipeline)
    }

    async fn find_pipeline_in_workspace_by_slug(
        &mut self,
        workspace_id: Uuid,
        slug: &str,
    ) -> Result<Option<WithAccountRef<WorkspacePipeline>>> {
        use schema::workspace_pipelines::dsl;
        use schema::{accounts, workspace_pipelines};

        let row = workspace_pipelines::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::slug.eq(slug))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspacePipeline::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(WorkspacePipeline, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn offset_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> Result<Vec<WorkspacePipeline>> {
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
            .map_err(Error::from)?;

        Ok(pipelines)
    }

    async fn cursor_list_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        status_filter: Option<PipelineStatus>,
        search_term: Option<&str>,
    ) -> Result<CursorPage<WithAccountRef<WorkspacePipeline>>> {
        use schema::workspace_pipelines::dsl;
        use schema::{accounts, workspace_pipelines};

        // Build base query with filters
        let mut base_query = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply status filter
        if let Some(status) = status_filter {
            base_query = base_query.filter(dsl::status.eq(status));
        }

        // Hybrid name search: ILIKE substring (works for short queries) OR
        // trigram similarity (typo tolerance); both served by the trgm index.
        if let Some(term) = search_term {
            base_query = base_query.filter(
                dsl::display_name
                    .ilike(ilike_contains(term))
                    .or(dsl::display_name.trgm_similar_to(term)),
            );
        }

        let total = if pagination.include_count {
            Some(
                base_query
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        // Rebuild query for fetching items
        let mut query = workspace_pipelines::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if let Some(status) = status_filter {
            query = query.filter(dsl::status.eq(status));
        }

        // Hybrid name search: ILIKE substring OR trigram similarity (see above).
        if let Some(term) = search_term {
            query = query.filter(
                dsl::display_name
                    .ilike(ilike_contains(term))
                    .or(dsl::display_name.trgm_similar_to(term)),
            );
        }

        let limit = pagination.fetch_limit();

        let rows: Vec<(WorkspacePipeline, AccountRefRow)> = if let Some(cursor) = &pagination.after
        {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select((
                    WorkspacePipeline::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                ))
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(Error::from)?
        } else {
            query
                .select((
                    WorkspacePipeline::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                ))
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(Error::from)?
        };

        let items: Vec<WithAccountRef<WorkspacePipeline>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            (wc.item.created_at.into(), wc.item.id)
        }))
    }

    async fn offset_list_account_pipelines(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> Result<Vec<WorkspacePipeline>> {
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
            .map_err(Error::from)?;

        Ok(pipelines)
    }

    async fn list_enabled_workspace_pipelines(
        &mut self,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipelines = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::status.eq(PipelineStatus::Enabled))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::display_name.asc())
            .select(WorkspacePipeline::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(pipelines)
    }

    async fn update_workspace_pipeline(
        &mut self,
        pipeline_id: Uuid,
        updates: UpdateWorkspacePipeline,
    ) -> Result<WorkspacePipeline> {
        use schema::workspace_pipelines::{self, dsl};

        let pipeline = diesel::update(workspace_pipelines::table.filter(dsl::id.eq(pipeline_id)))
            .set(&updates)
            .returning(WorkspacePipeline::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(pipeline)
    }

    async fn delete_workspace_pipeline(&mut self, pipeline_id: Uuid) -> Result<()> {
        use diesel::dsl::now;
        use schema::workspace_pipelines::{self, dsl};

        diesel::update(workspace_pipelines::table.filter(dsl::id.eq(pipeline_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn count_workspace_pipelines_by_status(
        &mut self,
        workspace_id: Uuid,
        status: PipelineStatus,
    ) -> Result<i64> {
        use schema::workspace_pipelines::{self, dsl};

        let count = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::status.eq(status))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(count)
    }

    async fn search_pipelines_by_name(
        &mut self,
        workspace_id: Uuid,
        search_term: &str,
        limit: i64,
    ) -> Result<Vec<WorkspacePipeline>> {
        use schema::workspace_pipelines::{self, dsl};

        let pipelines = workspace_pipelines::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(
                dsl::display_name
                    .ilike(ilike_contains(search_term))
                    .or(dsl::display_name.trgm_similar_to(search_term)),
            )
            .filter(dsl::deleted_at.is_null())
            .order(dsl::display_name.asc())
            .limit(limit)
            .select(WorkspacePipeline::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(pipelines)
    }
}
