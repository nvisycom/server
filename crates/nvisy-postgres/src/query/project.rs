//! Project repository for managing project operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProject, NewProjectMember, Project, ProjectMember, UpdateProject};
use crate::types::{ProjectRole, ProjectStatus, ProjectVisibility};
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for project database operations.
///
/// Handles project lifecycle management including creation, updates, status changes,
/// archiving, and search functionality.
pub trait ProjectRepository {
    /// Creates a new project.
    ///
    /// Inserts a new project record with the provided configuration.
    fn create_project(&self, project: NewProject)
    -> impl Future<Output = PgResult<Project>> + Send;

    /// Creates a new project and adds the creator as an admin member.
    ///
    /// Performs both operations in a single transaction to ensure atomic
    /// creation of the project and initial admin membership.
    fn create_project_with_admin(
        &self,
        project: NewProject,
        creator_id: Uuid,
    ) -> impl Future<Output = PgResult<(Project, ProjectMember)>> + Send;

    /// Finds a project by ID, excluding soft-deleted projects.
    fn find_project_by_id(
        &self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Project>>> + Send;

    /// Finds projects created by a user.
    ///
    /// Retrieves projects ordered by creation date with newest first.
    fn find_projects_by_creator(
        &self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;

    /// Updates a project with partial changes.
    fn update_project(
        &self,
        project_id: Uuid,
        changes: UpdateProject,
    ) -> impl Future<Output = PgResult<Project>> + Send;

    /// Soft deletes a project by setting the deletion timestamp.
    fn delete_project(&self, project_id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Archives a project by changing status from Active to Archived.
    fn archive_project(&self, project_id: Uuid) -> impl Future<Output = PgResult<Project>> + Send;

    /// Unarchives a project by changing status from Archived to Active.
    fn unarchive_project(&self, project_id: Uuid)
    -> impl Future<Output = PgResult<Project>> + Send;

    /// Lists projects with optional visibility and status filters.
    ///
    /// Returns projects ordered by update time with most recent first.
    fn list_projects(
        &self,
        visibility_filter: Option<ProjectVisibility>,
        status_filter: Option<ProjectStatus>,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;

    /// Searches public projects by name or description.
    ///
    /// Performs case-insensitive search across project names and descriptions.
    fn search_projects(
        &self,
        search_query: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;

    /// Finds projects with overlapping tags.
    fn find_projects_by_tags(
        &self,
        search_tags: &[String],
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Project>>> + Send;

    /// Gets project statistics: member count, pending invites, and activity count.
    ///
    /// Returns a tuple of (member_count, pending_invites, activity_count).
    fn get_project_stats(
        &self,
        project_id: Uuid,
    ) -> impl Future<Output = PgResult<(i64, i64, i64)>> + Send;

    /// Counts total non-deleted projects created by a user.
    fn get_user_project_count(&self, user_id: Uuid) -> impl Future<Output = PgResult<i64>> + Send;
}

impl ProjectRepository for PgClient {
    async fn create_project(&self, project: NewProject) -> PgResult<Project> {
        use schema::projects;

        let mut conn = self.get_connection().await?;
        let project = diesel::insert_into(projects::table)
            .values(&project)
            .returning(Project::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn create_project_with_admin(
        &self,
        project: NewProject,
        creator_id: Uuid,
    ) -> PgResult<(Project, ProjectMember)> {
        use diesel_async::AsyncConnection;
        use schema::{project_members, projects};

        let mut conn = self.get_connection().await?;

        conn.transaction(|conn| {
            Box::pin(async move {
                let project = diesel::insert_into(projects::table)
                    .values(&project)
                    .returning(Project::as_returning())
                    .get_result(conn)
                    .await
                    .map_err(PgError::from)?;

                let new_member = NewProjectMember {
                    project_id: project.id,
                    account_id: creator_id,
                    member_role: ProjectRole::Admin,
                    created_by: creator_id,
                    updated_by: creator_id,
                    ..Default::default()
                };

                let member = diesel::insert_into(project_members::table)
                    .values(&new_member)
                    .returning(ProjectMember::as_returning())
                    .get_result(conn)
                    .await
                    .map_err(PgError::from)?;

                Ok((project, member))
            })
        })
        .await
    }

    async fn find_project_by_id(&self, project_id: Uuid) -> PgResult<Option<Project>> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        let project = projects
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .select(Project::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn find_projects_by_creator(
        &self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        let project_list = projects
            .filter(created_by.eq(creator_id))
            .filter(deleted_at.is_null())
            .select(Project::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    async fn update_project(&self, project_id: Uuid, changes: UpdateProject) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .set(&changes)
            .returning(Project::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn delete_project(&self, project_id: Uuid) -> PgResult<()> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .set(deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn archive_project(&self, project_id: Uuid) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .filter(status.eq(ProjectStatus::Active))
            .set((
                status.eq(ProjectStatus::Archived),
                archived_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
            ))
            .returning(Project::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn unarchive_project(&self, project_id: Uuid) -> PgResult<Project> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        let project = diesel::update(projects)
            .filter(id.eq(project_id))
            .filter(deleted_at.is_null())
            .filter(status.eq(ProjectStatus::Archived))
            .set((
                status.eq(ProjectStatus::Active),
                archived_at.eq(None::<jiff_diesel::Timestamp>),
            ))
            .returning(Project::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project)
    }

    async fn list_projects(
        &self,
        visibility_filter: Option<ProjectVisibility>,
        status_filter: Option<ProjectStatus>,
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
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
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    async fn search_projects(
        &self,
        search_query: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
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
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    async fn find_projects_by_tags(
        &self,
        search_tags: &[String],
        pagination: Pagination,
    ) -> PgResult<Vec<Project>> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        let project_list = projects
            .filter(tags.overlaps_with(search_tags))
            .filter(deleted_at.is_null())
            .filter(status.ne(ProjectStatus::Suspended))
            .select(Project::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(project_list)
    }

    async fn get_project_stats(&self, project_id: Uuid) -> PgResult<(i64, i64, i64)> {
        use schema::{project_invites, project_members};

        let mut conn = self.get_connection().await?;

        let member_count: i64 = project_members::table
            .filter(project_members::project_id.eq(project_id))
            .filter(project_members::is_active.eq(true))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        let pending_invites: i64 = project_invites::table
            .filter(project_invites::project_id.eq(project_id))
            .filter(project_invites::invite_status.eq(crate::types::InviteStatus::Pending))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        let activity_count: i64 = 0;

        Ok((member_count, pending_invites, activity_count))
    }

    async fn get_user_project_count(&self, user_id: Uuid) -> PgResult<i64> {
        use schema::projects::dsl::*;

        let mut conn = self.get_connection().await?;
        let count: i64 = projects
            .filter(created_by.eq(user_id))
            .filter(deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }
}
