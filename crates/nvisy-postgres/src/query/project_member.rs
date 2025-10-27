//! Project member repository for managing project membership operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectMember, ProjectMember, UpdateProjectMember};
use crate::types::ProjectRole;
use crate::{PgError, PgResult, schema};

/// Repository for project member table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectMemberRepository;

impl ProjectMemberRepository {
    /// Creates a new project member repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Adds a new member to a project.
    pub async fn add_project_member(
        conn: &mut AsyncPgConnection,
        member: NewProjectMember,
    ) -> PgResult<ProjectMember> {
        use schema::project_members;

        let member = diesel::insert_into(project_members::table)
            .values(&member)
            .returning(ProjectMember::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(member)
    }

    /// Finds a specific project member.
    pub async fn find_project_member(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<Option<ProjectMember>> {
        use schema::project_members::dsl::*;

        let member = project_members
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .select(ProjectMember::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(member)
    }

    /// Updates a project member.
    pub async fn update_project_member(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateProjectMember,
    ) -> PgResult<ProjectMember> {
        use schema::project_members::dsl::*;

        let member = diesel::update(project_members)
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .set(&changes)
            .returning(ProjectMember::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(member)
    }

    /// Removes a member from a project.
    pub async fn remove_project_member(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        member_account_id: Uuid,
    ) -> PgResult<()> {
        use schema::project_members::dsl::*;

        diesel::delete(project_members)
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(member_account_id))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Lists all members of a project.
    pub async fn list_project_members(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .select(ProjectMember::as_select())
            .order((member_role.asc(), created_at.asc()))
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    /// Lists all projects a user is a member of.
    pub async fn list_user_projects(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

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
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(memberships)
    }

    /// Checks a user's role in a project.
    pub async fn check_user_role(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> PgResult<Option<ProjectRole>> {
        use schema::project_members::dsl::*;

        let role = project_members
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .filter(is_active.eq(true))
            .select(member_role)
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(role)
    }

    /// Updates the last access time for a member.
    pub async fn touch_member_access(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> PgResult<()> {
        use schema::project_members::dsl::*;

        diesel::update(project_members)
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .filter(is_active.eq(true))
            .set(last_accessed_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Finds members by their role in a project.
    pub async fn find_members_by_role(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        role: ProjectRole,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(role))
            .filter(is_active.eq(true))
            .select(ProjectMember::as_select())
            .order(created_at.asc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    /// Gets the count of members in a project.
    pub async fn get_member_count(conn: &mut AsyncPgConnection, proj_id: Uuid) -> PgResult<i64> {
        use schema::project_members::dsl::*;

        let count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Gets member count by role for a project.
    pub async fn get_member_count_by_role(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<(i64, i64, i64, i64)> {
        use schema::project_members::dsl::*;

        // Count owners
        let owner_count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(ProjectRole::Owner))
            .filter(is_active.eq(true))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count admins
        let admin_count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(ProjectRole::Admin))
            .filter(is_active.eq(true))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count editors
        let editor_count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(ProjectRole::Editor))
            .filter(is_active.eq(true))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count viewers
        let viewer_count: i64 = project_members
            .filter(project_id.eq(proj_id))
            .filter(member_role.eq(ProjectRole::Viewer))
            .filter(is_active.eq(true))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok((owner_count, admin_count, editor_count, viewer_count))
    }

    /// Checks if a user has access to a project.
    pub async fn check_project_access(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        user_id: Uuid,
    ) -> PgResult<bool> {
        use schema::project_members::dsl::*;

        let is_member = project_members
            .filter(project_id.eq(proj_id))
            .filter(account_id.eq(user_id))
            .filter(is_active.eq(true))
            .select(account_id)
            .first::<Uuid>(conn)
            .await
            .optional()
            .map_err(PgError::from)?
            .is_some();

        Ok(is_member)
    }

    /// Gets members who have favorited the project.
    pub async fn get_favorite_members(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_favorite.eq(true))
            .filter(is_active.eq(true))
            .select(ProjectMember::as_select())
            .order(created_at.asc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    /// Gets members who have notifications enabled.
    pub async fn get_notifiable_members(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        notification_type: &str,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let mut query = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .into_boxed();

        // Filter by notification type
        match notification_type {
            "updates" => query = query.filter(notify_updates.eq(true)),
            "comments" => query = query.filter(notify_comments.eq(true)),
            "mentions" => query = query.filter(notify_mentions.eq(true)),
            _ => {} // No additional filter
        }

        let members = query
            .select(ProjectMember::as_select())
            .order(created_at.asc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }

    /// Gets recently active members in a project.
    pub async fn get_recently_active_members(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        hours: i64,
    ) -> PgResult<Vec<ProjectMember>> {
        use schema::project_members::dsl::*;

        let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(hours);

        let members = project_members
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .filter(last_accessed_at.gt(cutoff_time))
            .select(ProjectMember::as_select())
            .order(last_accessed_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(members)
    }
}
