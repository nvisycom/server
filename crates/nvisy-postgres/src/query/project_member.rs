//! Project member repository for managing project membership operations.
//!
//! This module provides comprehensive database operations for managing project memberships,
//! including member addition/removal, role management, access control, and activity tracking.
//! It handles the full lifecycle of project memberships from invitation to removal.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectMember, Project, ProjectMember, UpdateProjectMember};
use crate::types::ProjectRole;
use crate::{PgError, PgResult, schema};

/// Repository for project member table operations.
///
/// Provides comprehensive database operations for managing project memberships,
/// including CRUD operations, role-based access control, and activity tracking.
/// This repository handles all database interactions related to project membership
/// management and permission checking.
///
/// The repository is stateless and thread-safe, designed to be used as a singleton
/// or instantiated as needed. All methods require an active database connection
/// and return results wrapped in the standard `PgResult` type for error handling.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectMemberRepository;

impl ProjectMemberRepository {
    /// Creates a new project member repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `ProjectMemberRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Adds a new member to a project with the specified role and preferences.
    ///
    /// Creates a new project membership record with the provided configuration.
    /// The member is automatically assigned default preferences and notification
    /// settings, which can be customized later through updates.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `member` - Member data including project_id, account_id, role, and preferences
    ///
    /// # Returns
    ///
    /// Returns the created `ProjectMember` with all generated fields populated,
    /// or a `PgError` if the addition fails due to constraints or database issues.
    ///
    /// # Errors
    ///
    /// - `ConstraintViolation` - Member already exists or foreign key violations
    /// - `DatabaseError` - Connection issues or query execution failures
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

    /// Retrieves a specific project member by project and account identifiers.
    ///
    /// Fetches the complete membership record including role, preferences,
    /// and activity information. This method is commonly used for permission
    /// checking and member profile display.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `member_account_id` - Unique identifier of the member's account
    ///
    /// # Returns
    ///
    /// Returns the `ProjectMember` if found, or `None` if no membership exists
    /// between the specified project and account.
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

    /// Updates an existing project member's configuration and preferences.
    ///
    /// Modifies member settings such as role, notification preferences, or
    /// custom permissions. Only provided fields are updated, allowing for
    /// partial updates. The updated_at timestamp is automatically maintained.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `member_account_id` - Unique identifier of the member's account
    /// * `changes` - Update data containing fields to modify
    ///
    /// # Returns
    ///
    /// Returns the updated `ProjectMember` with all modifications applied.
    /// Fails if no membership exists between the specified project and account.
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

    /// Permanently removes a member from a project.
    ///
    /// Deletes the membership record, effectively revoking all access to the project.
    /// This operation is irreversible and should be used carefully. Consider
    /// deactivating the member instead of removing for audit trail preservation.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `member_account_id` - Unique identifier of the member's account
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the member was removed successfully.
    /// Succeeds even if no membership existed.
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

    /// Retrieves all active members of a specific project with pagination.
    ///
    /// Fetches project members ordered by role hierarchy and creation date.
    /// Only active members are included in the results. This method is
    /// commonly used for member management interfaces and permission audits.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `pagination` - Pagination parameters for limiting results
    ///
    /// # Returns
    ///
    /// Returns a vector of active `ProjectMember` instances for the project,
    /// ordered by role priority and creation time.
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

    /// Retrieves all projects where a user has active membership.
    ///
    /// Fetches user's project memberships ordered by favorites, recent activity,
    /// and creation date. This provides a personalized project list for user
    /// interfaces, prioritizing frequently accessed and favorited projects.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `user_id` - Unique identifier of the user account
    /// * `pagination` - Pagination parameters for limiting results
    ///
    /// # Returns
    ///
    /// Returns a vector of `ProjectMember` instances representing the user's
    /// active memberships, ordered by preference and activity.
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

    /// Lists all projects a user is a member of with project details.
    ///
    /// This method performs a single query with a JOIN to fetch both the project
    /// and membership data, avoiding N+1 query problems.
    pub async fn list_user_projects_with_details(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<(Project, ProjectMember)>> {
        use schema::{project_members, projects};

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
            .load::<(Project, ProjectMember)>(conn)
            .await
            .map_err(PgError::from)?;

        Ok(results)
    }

    /// Retrieves a user's role in a specific project for permission checking.
    ///
    /// Determines the user's access level within a project by fetching their
    /// assigned role. This is essential for authorization and feature access
    /// control throughout the application.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `user_id` - Unique identifier of the user account
    ///
    /// # Returns
    ///
    /// Returns the user's `ProjectRole` if they are an active member,
    /// or `None` if they have no access to the project.
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

    /// Records the current timestamp as the member's last access time.
    ///
    /// Updates the last_accessed_at field to track member activity within
    /// projects. This information is used for activity monitoring, member
    /// engagement analysis, and prioritizing projects in user interfaces.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `user_id` - Unique identifier of the user account
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the timestamp was updated successfully.
    /// No-op if the user is not an active member of the project.
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

    /// Retrieves all project members with a specific role.
    ///
    /// Filters project members by their assigned role, useful for role-based
    /// operations like notifications to administrators or permission audits.
    /// Results are ordered by membership creation date.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `role` - The project role to filter by
    ///
    /// # Returns
    ///
    /// Returns a vector of `ProjectMember` instances with the specified role,
    /// ordered by membership creation time.
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

    /// Counts the total number of active members in a project.
    ///
    /// Provides a simple count of active memberships for capacity planning,
    /// billing calculations, and project statistics. Only active members
    /// are included in the count.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    ///
    /// # Returns
    ///
    /// Returns the count of active members as a 64-bit integer.
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

    /// Counts active members in a project grouped by their roles.
    ///
    /// Provides detailed membership statistics by role for project management
    /// dashboards, permission audits, and organizational analysis. Counts are
    /// returned in a tuple ordered by role hierarchy.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    ///
    /// # Returns
    ///
    /// Returns a tuple of (owners, admins, editors, viewers) counts.
    /// All counts represent active members only.
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

    /// Determines if a user has any level of access to a project.
    ///
    /// Performs a simple membership check to determine if the user is an active
    /// member of the project, regardless of role. This is useful for basic
    /// access control before checking specific permissions.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `user_id` - Unique identifier of the user account
    ///
    /// # Returns
    ///
    /// Returns `true` if the user is an active member, `false` otherwise.
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

    /// Retrieves all members who have marked the project as a favorite.
    ///
    /// Fetches members who have enabled the favorite flag for this project,
    /// indicating high engagement or priority. This is useful for targeted
    /// communications and understanding project popularity.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    ///
    /// # Returns
    ///
    /// Returns a vector of `ProjectMember` instances who have favorited
    /// the project, ordered by membership creation time.
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

    /// Retrieves members who should receive notifications of a specific type.
    ///
    /// Filters project members based on their notification preferences for
    /// targeted communication delivery. Supports filtering by notification
    /// type such as updates, comments, or mentions.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `notification_type` - Type of notification ("updates", "comments", "mentions")
    ///
    /// # Returns
    ///
    /// Returns a vector of `ProjectMember` instances who have enabled
    /// notifications for the specified type.
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

    /// Retrieves members who have accessed the project within a time window.
    ///
    /// Finds project members who have been active within the specified number
    /// of hours, based on their last_accessed_at timestamp. This is useful
    /// for engagement analysis and targeted communications to active users.
    ///
    /// # Arguments
    ///
    /// * `conn` - Database connection for executing the query
    /// * `proj_id` - Unique identifier of the project
    /// * `hours` - Number of hours to look back for activity
    ///
    /// # Returns
    ///
    /// Returns a vector of recently active `ProjectMember` instances,
    /// ordered by most recent access time.
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
