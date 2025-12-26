//! Project member repository for managing project membership.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectMember, Project, ProjectMember, UpdateProjectMember};
use crate::types::ProjectRole;
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for project member database operations.
///
/// Handles project membership management including CRUD operations, role-based
/// access control, and activity tracking.
pub trait ProjectMemberRepository {
    /// Adds a new member to a project.
    fn add_project_member(
        &self,
        member: NewProjectMember,
    ) -> impl Future<Output = PgResult<ProjectMember>> + Send;

    /// Finds a project member by project and account IDs.
    fn find_project_member(
        &self,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectMember>>> + Send;

    /// Updates a project member with partial changes.
    fn update_project_member(
        &self,
        proj_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateProjectMember,
    ) -> impl Future<Output = PgResult<ProjectMember>> + Send;

    /// Permanently removes a member from a project.
    fn remove_project_member(
        &self,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists active members of a project.
    ///
    /// Returns members ordered by role and creation date.
    fn list_project_members(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectMember>>> + Send;

    /// Lists projects where a user is a member.
    ///
    /// Returns memberships ordered by favorites and recent activity.
    fn list_user_projects(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectMember>>> + Send;

    /// Lists user projects with full project details via JOIN.
    fn list_user_projects_with_details(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<(Project, ProjectMember)>>> + Send;

    /// Gets a user's role in a project for permission checking.
    ///
    /// Returns the role if the user is an active member, None otherwise.
    fn check_user_role(
        &self,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectRole>>> + Send;

    /// Updates the last access timestamp for a member.
    fn touch_member_access(
        &self,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Finds all active members with a specific role.
    fn find_members_by_role(
        &self,
        proj_id: Uuid,
        role: ProjectRole,
    ) -> impl Future<Output = PgResult<Vec<ProjectMember>>> + Send;

    /// Counts total active members in a project.
    fn get_member_count(&self, proj_id: Uuid) -> impl Future<Output = PgResult<i64>> + Send;

    /// Counts active members grouped by role.
    ///
    /// Returns a tuple of (admins, editors, viewers).
    fn get_member_count_by_role(
        &self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<(i64, i64, i64)>> + Send;

    /// Checks if a user has any access to a project.
    fn check_project_access(
        &self,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Finds members who have favorited the project.
    fn get_favorite_members(
        &self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ProjectMember>>> + Send;

    /// Finds members who have enabled a specific notification type.
    fn get_notifiable_members(
        &self,
        proj_id: Uuid,
        notification_type: &str,
    ) -> impl Future<Output = PgResult<Vec<ProjectMember>>> + Send;

    /// Finds members who accessed the project within the specified hours.
    fn get_recently_active_members(
        &self,
        proj_id: Uuid,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<ProjectMember>>> + Send;
}

impl ProjectMemberRepository for PgClient {
    async fn add_project_member(&self, member: NewProjectMember) -> PgResult<ProjectMember> {
        use schema::project_members;

        let mut conn = self.get_connection().await?;
        let member = diesel::insert_into(project_members::table)
            .values(&member)
            .returning(ProjectMember::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn find_project_member(
        &self,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<Option<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let member = project_members
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .select(ProjectMember::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn update_project_member(
        &self,
        proj_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateProjectMember,
    ) -> PgResult<ProjectMember> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let member = diesel::update(project_members)
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .set(&changes)
            .returning(ProjectMember::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(member)
    }

    async fn remove_project_member(&self, proj_id: Uuid, member_account_id: Uuid) -> PgResult<()> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        diesel::delete(project_members)
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_project_members(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .select(ProjectMember::as_select())
            .order((member_role.asc(), created_at.asc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn list_user_projects(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let memberships = project_members
            .filter(account_id.eq(user_id))
            .filter(is_active.eq(true))
            .select(ProjectMember::as_select())
            .order((
                is_favorite.desc(),
                last_accessed_at.desc().nulls_last(),
                created_at.desc(),
            ))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(memberships)
    }

    async fn list_user_projects_with_details(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<(Project, ProjectMember)>> {
        use schema::{project_members, projects};

        let mut conn = self.get_connection().await?;
        let results = project_members::table
            .inner_join(projects::table.on(projects::id.eq(project_members::project_id)))
            .filter(project_members::account_id.eq(user_id))
            .filter(project_members::is_active.eq(true))
            .filter(projects::deleted_at.is_null())
            .select((Project::as_select(), ProjectMember::as_select()))
            .order((
                project_members::is_favorite.desc(),
                project_members::last_accessed_at.desc().nulls_last(),
                project_members::created_at.desc(),
            ))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load::<(Project, ProjectMember)>(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(results)
    }

    async fn check_user_role(&self, proj_id: Uuid, user_id: Uuid) -> PgResult<Option<ProjectRole>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let role = project_members
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .filter(is_active.eq(true))
            .select(member_role)
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(role)
    }

    async fn touch_member_access(&self, proj_id: Uuid, user_id: Uuid) -> PgResult<()> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        diesel::update(project_members)
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .filter(is_active.eq(true))
            .set(last_accessed_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn find_members_by_role(
        &self,
        proj_id: Uuid,
        role: ProjectRole,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(role))
            .filter(is_active.eq(true))
            .select(ProjectMember::as_select())
            .order(created_at.asc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn get_member_count(&self, proj_id: Uuid) -> PgResult<i64> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn get_member_count_by_role(&self, proj_id: Uuid) -> PgResult<(i64, i64, i64)> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;

        let admin_count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(ProjectRole::Admin))
            .filter(is_active.eq(true))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        let editor_count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(ProjectRole::Editor))
            .filter(is_active.eq(true))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        let viewer_count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(ProjectRole::Viewer))
            .filter(is_active.eq(true))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok((admin_count, editor_count, viewer_count))
    }

    async fn check_project_access(&self, proj_id: Uuid, user_id: Uuid) -> PgResult<bool> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let is_member = project_members
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .filter(is_active.eq(true))
            .select(account_id)
            .first::<Uuid>(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?
            .is_some();

        Ok(is_member)
    }

    async fn get_favorite_members(&self, proj_id: Uuid) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_favorite.eq(true))
            .filter(is_active.eq(true))
            .select(ProjectMember::as_select())
            .order(created_at.asc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn get_notifiable_members(
        &self,
        proj_id: Uuid,
        notification_type: &str,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let mut query = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .into_boxed();

        match notification_type {
            "updates" => query = query.filter(notify_updates.eq(true)),
            "comments" => query = query.filter(notify_comments.eq(true)),
            "mentions" => query = query.filter(notify_mentions.eq(true)),
            _ => {}
        }

        let members = query
            .select(ProjectMember::as_select())
            .order(created_at.asc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    async fn get_recently_active_members(
        &self,
        proj_id: Uuid,
        hours: i64,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut conn = self.get_connection().await?;
        let cutoff_time = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(hours));

        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .filter(last_accessed_at.gt(cutoff_time))
            .select(ProjectMember::as_select())
            .order(last_accessed_at.desc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }
}
