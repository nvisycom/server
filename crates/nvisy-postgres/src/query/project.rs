//! Project repository for managing project operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProject, Project, UpdateProject};
use crate::types::{ProjectStatus, ProjectVisibility};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for project database operations.
///
/// Handles project lifecycle management including creation, updates, status changes,
/// archiving, and search functionality.
pub trait ProjectRepository {
    /// Creates a new project.
    ///
    /// Inserts a new project record with the provided configuration.
    fn create_project(
        &mut self,
        project: NewProject,
    ) -> impl Future<Output = PgResult<Project>> + Send;

    /// Finds a project by ID, excluding soft-deleted projects.
    fn find_project_by_id(
        &mut self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Project>>> + Send;

    /// Finds projects created by a user.
    ///
    /// Retrieves projects ordered by creation date with newest first.
    fn find_projects_by_creator(
        &mut self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;

    /// Updates a project with partial changes.
    fn update_project(
        &mut self,
        project_id: Uuid,
        changes: UpdateProject,
    ) -> impl Future<Output = PgResult<Project>> + Send;

    /// Soft deletes a project by setting the deletion timestamp.
    fn delete_project(&mut self, project_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Archives a project by changing status from Active to Archived.
    fn archive_project(
        &mut self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<Project>> + Send;

    /// Unarchives a project by changing status from Archived to Active.
    fn unarchive_project(
        &mut self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<Project>> + Send;

    /// Lists projects with optional visibility and status filters.
    ///
    /// Returns projects ordered by update time with most recent first.
    fn list_projects(
        &mut self,
        visibility_filter: Option<ProjectVisibility>,
        status_filter: Option<ProjectStatus>,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;

    /// Searches public projects by name or description.
    ///
    /// Performs case-insensitive search across project names and descriptions.
    fn search_projects(
        &mut self,
        search_query: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;

    /// Finds projects with overlapping tags.
    fn find_projects_by_tags(
        &mut self,
        search_tags: &[String],
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;
}

impl ProjectRepository for PgConnection {
    async fn create_project(&mut self, project: NewProject) -> PgResult<Project> {
        use schema::projects;

        let project = diesel::insert_into(projects::table)
            .values(&project)
            .returning(Project::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn find_project_by_id(&mut self, project_id: Uuid) -> PgResult<Option<Project>> {
        use schema::projects::dsl::*;

        let project = projects
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .select(Project::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn find_projects_by_creator(
        &mut self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let project_list = projects
            .filter(created_by.eq(creator_id))
            .filter(deleted_at.is_null())
            .select(Project::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    async fn update_project(
        &mut self,
        project_id: Uuid,
        changes: UpdateProject,
    ) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .set(&changes)
            .returning(Project::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn delete_project(&mut self, project_id: Uuid) -> PgResult<()> {
        use schema::projects::dsl::*;

        diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .set(deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn archive_project(&mut self, project_id: Uuid) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .filter(status.eq(ProjectStatus::Active))
            .set((
                status.eq(ProjectStatus::Archived),
                archived_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
            ))
            .returning(Project::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn unarchive_project(&mut self, project_id: Uuid) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .filter(status.eq(ProjectStatus::Archived))
            .set((
                status.eq(ProjectStatus::Active),
                archived_at.eq(None::<jiff_diesel::Timestamp>),
            ))
            .returning(Project::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn list_projects(
        &mut self,
        visibility_filter: Option<ProjectVisibility>,
        status_filter: Option<ProjectStatus>,
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let mut query = projects.filter(deleted_at.is_null()).into_boxed();

        if let Some(vis) = visibility_filter {
            query = query.filter(visibility.eq(vis));
        }

        if let Some(stat) = status_filter {
            query = query.filter(status.eq(stat));
        }

        let project_list = query
            .select(Project::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    async fn search_projects(
        &mut self,
        search_query: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let search_pattern = format!("%{}%", search_query);

        let project_list = projects
            .filter(deleted_at.is_null())
            .filter(diesel::BoolExpressionMethods::or(
                display_name.ilike(&search_pattern),
                description.ilike(&search_pattern),
            ))
            .filter(visibility.eq(ProjectVisibility::Public))
            .select(Project::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    async fn find_projects_by_tags(
        &mut self,
        search_tags: &[String],
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let project_list = projects
            .filter(tags.overlaps_with(search_tags))
            .filter(deleted_at.is_null())
            .filter(status.ne(ProjectStatus::Suspended))
            .select(Project::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }
}
