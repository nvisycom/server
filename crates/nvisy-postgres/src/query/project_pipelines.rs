//! Project pipeline repository for managing project pipeline operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectPipeline, ProjectPipeline, UpdateProjectPipeline};
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for project pipeline database operations.
///
/// Handles pipeline management including CRUD operations, default pipeline
/// management, and pipeline type filtering.
pub trait ProjectPipelineRepository {
    /// Creates a new project pipeline.
    fn create_project_pipeline(
        &self,
        new_pipeline: NewProjectPipeline,
    ) -> impl Future<Output = PgResult<ProjectPipeline>> + Send;

    /// Finds a project pipeline by ID.
    fn find_project_pipeline_by_id(
        &self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectPipeline>>> + Send;

    /// Lists all pipelines for a project.
    fn list_project_pipelines(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectPipeline>>> + Send;

    /// Lists active pipelines for a project.
    fn list_active_project_pipelines(
        &self,
        project_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectPipeline>>> + Send;

    /// Finds pipelines by type across all projects.
    fn find_pipelines_by_type(
        &self,
        pipeline_type: String,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectPipeline>>> + Send;

    /// Finds the default pipeline for a project and type.
    fn find_default_pipeline(
        &self,
        project_id: Uuid,
        pipeline_type: String,
    ) -> impl Future<Output = PgResult<Option<ProjectPipeline>>> + Send;

    /// Finds pipelines by project and type.
    fn find_project_pipelines_by_type(
        &self,
        project_id: Uuid,
        pipeline_type: String,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectPipeline>>> + Send;

    /// Updates a project pipeline.
    fn update_project_pipeline(
        &self,
        pipeline_id: Uuid,
        changes: UpdateProjectPipeline,
    ) -> impl Future<Output = PgResult<ProjectPipeline>> + Send;

    /// Soft deletes a project pipeline.
    fn delete_project_pipeline(
        &self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Counts total pipelines for a project.
    fn count_project_pipelines(
        &self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Counts active pipelines for a project.
    fn count_active_project_pipelines(
        &self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Sets a pipeline as the default for its type in a project.
    fn set_pipeline_as_default(
        &self,
        pipeline_id: Uuid,
    ) -> impl Future<Output = PgResult<ProjectPipeline>> + Send;

    /// Lists all pipelines created by a specific account.
    fn list_pipelines_by_creator(
        &self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectPipeline>>> + Send;

    /// Checks if a pipeline exists with the given name in a project.
    fn pipeline_name_exists_in_project(
        &self,
        project_id: Uuid,
        pipeline_name: &str,
        exclude_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

/// Default implementation of ProjectPipelineRepository using AsyncPgConnection.
impl ProjectPipelineRepository for PgClient {
    async fn create_project_pipeline(
        &self,
        new_pipeline: NewProjectPipeline,
    ) -> PgResult<ProjectPipeline> {
        use schema::project_pipelines;

        let mut conn = self.get_connection().await?;
        let pipeline = diesel::insert_into(project_pipelines::table)
            .values(&new_pipeline)
            .returning(ProjectPipeline::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn find_project_pipeline_by_id(
        &self,
        pipeline_id: Uuid,
    ) -> PgResult<Option<ProjectPipeline>> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipeline = project_pipelines
            .filter(id.eq(pipeline_id))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn list_project_pipelines(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectPipeline>> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipelines = project_pipelines
            .filter(project_id.eq(proj_id))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn list_active_project_pipelines(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectPipeline>> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipelines = project_pipelines
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn find_pipelines_by_type(
        &self,
        pipeline_type_filter: String,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectPipeline>> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipelines = project_pipelines
            .filter(pipeline_type.eq(pipeline_type_filter))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn find_default_pipeline(
        &self,
        proj_id: Uuid,
        pipeline_type_filter: String,
    ) -> PgResult<Option<ProjectPipeline>> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipeline = project_pipelines
            .filter(project_id.eq(proj_id))
            .filter(pipeline_type.eq(pipeline_type_filter))
            .filter(is_default.eq(true))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn find_project_pipelines_by_type(
        &self,
        proj_id: Uuid,
        pipeline_type_filter: String,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectPipeline>> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipelines = project_pipelines
            .filter(project_id.eq(proj_id))
            .filter(pipeline_type.eq(pipeline_type_filter))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .order((is_default.desc(), created_at.desc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn update_project_pipeline(
        &self,
        pipeline_id: Uuid,
        changes: UpdateProjectPipeline,
    ) -> PgResult<ProjectPipeline> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipeline = diesel::update(project_pipelines)
            .filter(id.eq(pipeline_id))
            .set(&changes)
            .returning(ProjectPipeline::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(pipeline)
    }

    async fn delete_project_pipeline(&self, pipeline_id: Uuid) -> PgResult<()> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        diesel::update(project_pipelines)
            .filter(id.eq(pipeline_id))
            .set(deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn count_project_pipelines(&self, proj_id: Uuid) -> PgResult<i64> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let count = project_pipelines
            .filter(project_id.eq(proj_id))
            .filter(deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn count_active_project_pipelines(&self, proj_id: Uuid) -> PgResult<i64> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let count = project_pipelines
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .filter(deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn set_pipeline_as_default(&self, pipeline_id: Uuid) -> PgResult<ProjectPipeline> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;

        // First, get the pipeline to know its project and type
        let pipeline = project_pipelines
            .filter(id.eq(pipeline_id))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .first(&mut conn)
            .await
            .map_err(PgError::from)?;

        // Unset any existing default for the same project and type
        diesel::update(project_pipelines)
            .filter(project_id.eq(pipeline.project_id))
            .filter(pipeline_type.eq(&pipeline.pipeline_type))
            .filter(is_default.eq(true))
            .filter(deleted_at.is_null())
            .set(is_default.eq(false))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        // Set this pipeline as default
        let updated_pipeline = diesel::update(project_pipelines)
            .filter(id.eq(pipeline_id))
            .set(is_default.eq(true))
            .returning(ProjectPipeline::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(updated_pipeline)
    }

    async fn list_pipelines_by_creator(
        &self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectPipeline>> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let pipelines = project_pipelines
            .filter(created_by.eq(creator_id))
            .filter(deleted_at.is_null())
            .select(ProjectPipeline::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(pipelines)
    }

    async fn pipeline_name_exists_in_project(
        &self,
        proj_id: Uuid,
        pipeline_name: &str,
        exclude_id: Option<Uuid>,
    ) -> PgResult<bool> {
        use schema::project_pipelines::dsl::*;

        let mut conn = self.get_connection().await?;
        let mut query = project_pipelines
            .filter(project_id.eq(proj_id))
            .filter(display_name.eq(pipeline_name))
            .filter(deleted_at.is_null())
            .into_boxed();

        if let Some(exclude_pipeline_id) = exclude_id {
            query = query.filter(id.ne(exclude_pipeline_id));
        }

        let count = query
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}
