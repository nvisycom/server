//! Project repository for managing main project operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProject, Project, UpdateProject};
use crate::types::{ProjectStatus, ProjectVisibility};
use crate::{PgError, PgResult, schema};

/// Repository for main project table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectRepository;

impl ProjectRepository {
    /// Creates a new project repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new project in the database.
    pub async fn create_project(
        conn: &mut AsyncPgConnection,
        project: NewProject,
    ) -> PgResult<Project> {
        use schema::projects;

        let project = diesel::insert_into(projects::table)
            .values(&project)
            .returning(Project::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    /// Finds a project by its ID.
    pub async fn find_project_by_id(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
    ) -> PgResult<Option<Project>> {
        use schema::projects::dsl::*;

        let project = projects
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .select(Project::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(project)
    }

    /// Finds projects created by a specific user.
    pub async fn find_projects_by_creator(
        conn: &mut AsyncPgConnection,
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
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    /// Updates a project.
    pub async fn update_project(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        changes: UpdateProject,
    ) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .set(&changes)
            .returning(Project::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    /// Soft deletes a project.
    pub async fn delete_project(conn: &mut AsyncPgConnection, project_id: Uuid) -> PgResult<()> {
        use schema::projects::dsl::*;

        diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .set(deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Archives a project.
    pub async fn archive_project(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
    ) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .filter(status.eq(ProjectStatus::Active))
            .set((
                status.eq(ProjectStatus::Archived),
                archived_at.eq(Some(OffsetDateTime::now_utc())),
            ))
            .returning(Project::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    /// Unarchives a project.
    pub async fn unarchive_project(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
    ) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .filter(status.eq(ProjectStatus::Archived))
            .set((
                status.eq(ProjectStatus::Active),
                archived_at.eq(None::<OffsetDateTime>),
            ))
            .returning(Project::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    /// Lists projects with pagination and optional filtering.
    pub async fn list_projects(
        conn: &mut AsyncPgConnection,
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
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    /// Searches projects by name or description.
    pub async fn search_projects(
        conn: &mut AsyncPgConnection,
        search_query: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let search_pattern = format!("%{}%", search_query);

        let project_list = projects
            .filter(deleted_at.is_null())
            .filter(
                display_name
                    .ilike(&search_pattern)
                    .or(description.ilike(&search_pattern)),
            )
            .filter(visibility.eq(ProjectVisibility::Public))
            .select(Project::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    /// Lists all available project templates.
    pub async fn list_project_templates(conn: &mut AsyncPgConnection) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let templates = projects
            .filter(deleted_at.is_null())
            .filter(status.eq(ProjectStatus::Template))
            .select(Project::as_select())
            .order(display_name.asc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(templates)
    }

    /// Finds projects by tags.
    pub async fn find_projects_by_tags(
        conn: &mut AsyncPgConnection,
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
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    /// Gets basic project statistics.
    pub async fn get_project_stats(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
    ) -> PgResult<(i64, i64, i64)> {
        use schema::{project_invites, project_members};

        // Count active members
        let member_count: i64 = project_members::table
            .filter(project_members::project_id.eq(project_id))
            .filter(project_members::is_active.eq(true))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count pending invites
        let pending_invites: i64 = project_invites::table
            .filter(project_invites::project_id.eq(project_id))
            .filter(project_invites::invite_status.eq(crate::types::InviteStatus::Pending))
            .filter(project_invites::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count total activity (placeholder for now)
        let activity_count: i64 = 0; // Would need to implement activity counting

        Ok((member_count, pending_invites, activity_count))
    }

    /// Gets the number of projects created by a user.
    pub async fn get_user_project_count(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
    ) -> PgResult<i64> {
        use schema::projects::dsl::*;

        let count: i64 = projects
            .filter(created_by.eq(user_id))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Gets project creation statistics for a user.
    pub async fn get_user_project_stats(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
    ) -> PgResult<(i64, i64, i64)> {
        use schema::projects::dsl::*;

        // Total projects created
        let total_created: i64 = projects
            .filter(created_by.eq(user_id))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Active projects
        let active_count: i64 = projects
            .filter(created_by.eq(user_id))
            .filter(status.eq(ProjectStatus::Active))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Archived projects
        let archived_count: i64 = projects
            .filter(created_by.eq(user_id))
            .filter(status.eq(ProjectStatus::Archived))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok((total_created, active_count, archived_count))
    }
}
