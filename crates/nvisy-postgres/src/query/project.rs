//! Project repository for managing main project operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProject, Project, UpdateProject};
use crate::types::{ProjectStatus, ProjectVisibility};
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive project database operations.
///
/// Provides database operations for managing projects throughout their lifecycle,
/// including creation, updates, status management, search functionality, and
/// analytics. This repository handles all database interactions related to
/// project management and serves as the primary interface for project data.
///
/// The repository supports project visibility controls, status management,
/// archiving operations, and comprehensive search and filtering capabilities
/// to enable rich project management experiences.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectRepository;

impl ProjectRepository {
    /// Creates a new project repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `ProjectRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new project in the database with complete initial setup.
    ///
    /// Initializes a new project with the provided configuration and metadata.
    /// The project is immediately available for collaboration and can be found
    /// through various query methods. This is the primary method for project
    /// creation and onboarding.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project` - Complete project data including name, description, and settings
    ///
    /// # Returns
    ///
    /// The created `Project` with database-generated ID and timestamps,
    /// or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Project becomes immediately available for member invitations
    /// - Creator automatically becomes project owner
    /// - Project appears in relevant search and discovery interfaces
    /// - Enables collaborative workflows and resource sharing
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

    /// Finds a project by its unique identifier.
    ///
    /// Retrieves a specific project using its UUID, automatically excluding
    /// soft-deleted projects. This is the primary method for accessing
    /// individual projects when you know the exact project ID.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `project_id` - UUID of the project to retrieve
    ///
    /// # Returns
    ///
    /// The matching `Project` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
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

    /// Finds projects created by a specific user with pagination support.
    ///
    /// Retrieves a paginated list of projects where the specified user is
    /// the original creator. Results are ordered by creation date with
    /// newest projects first, providing a chronological view of the
    /// user's project creation history.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `creator_id` - UUID of the user whose created projects to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Project` entries created by the user, ordered by
    /// creation date (newest first), or a database error if the query fails.
    ///
    /// # User Dashboard Use Cases
    ///
    /// - "My Projects" dashboard sections
    /// - User profile project listings
    /// - Creator portfolio displays
    /// - Administrative user project oversight
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

    /// Updates a project with new information and settings.
    ///
    /// Applies partial updates to an existing project using the provided
    /// update structure. Only fields set to `Some(value)` will be modified,
    /// while `None` fields remain unchanged. The updated_at timestamp is
    /// automatically updated to reflect the modification time.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project_id` - UUID of the project to update
    /// * `changes` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `Project` with new values and timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Common Update Scenarios
    ///
    /// - Changing project names or descriptions
    /// - Updating visibility settings
    /// - Modifying project tags and metadata
    /// - Administrative project adjustments
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

    /// Soft deletes a project by setting the deletion timestamp.
    ///
    /// Marks a project as deleted without permanently removing it from the
    /// database. This preserves data for audit purposes and compliance
    /// requirements while preventing the project from appearing in normal
    /// queries and user interfaces.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project_id` - UUID of the project to soft delete
    ///
    /// # Returns
    ///
    /// `()` on successful deletion, or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Project immediately becomes inaccessible to users
    /// - All project data is preserved for audit and recovery
    /// - Related entities (members, documents, activities) may need cleanup
    /// - Project no longer appears in search or discovery interfaces
    ///
    /// # Important Considerations
    ///
    /// Consider the impact on project members and related resources
    /// before performing this operation. Implement proper cleanup
    /// procedures for associated data.
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

    /// Archives a project to preserve it in read-only state.
    ///
    /// Changes the project status from Active to Archived and sets the
    /// archive timestamp. Archived projects are preserved for reference
    /// but typically have restricted functionality and limited visibility
    /// in active project listings.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project_id` - UUID of the project to archive
    ///
    /// # Returns
    ///
    /// The updated `Project` with archived status and timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Archival Benefits
    ///
    /// - Preserves project data and history for future reference
    /// - Reduces clutter in active project listings
    /// - Maintains compliance with data retention policies
    /// - Enables project lifecycle management
    ///
    /// # Status Requirements
    ///
    /// Only Active projects can be archived. The operation will fail
    /// if the project is already archived or in another state.
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

    /// Unarchives a project to restore it to active status.
    ///
    /// Changes the project status from Archived back to Active and clears
    /// the archive timestamp. This restores full project functionality
    /// and makes the project visible in active project listings again.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project_id` - UUID of the project to unarchive
    ///
    /// # Returns
    ///
    /// The updated `Project` with active status and cleared archive timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Restoration Benefits
    ///
    /// - Restores full project functionality and collaboration
    /// - Makes project visible in active listings and search
    /// - Enables continued development and updates
    /// - Supports project lifecycle management
    ///
    /// # Status Requirements
    ///
    /// Only Archived projects can be unarchived. The operation will fail
    /// if the project is not currently in archived state.
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

    /// Lists projects with comprehensive filtering and pagination support.
    ///
    /// Retrieves a paginated list of projects with optional filtering by
    /// visibility and status. Results are ordered by last update time to
    /// show the most recently active projects first, providing an up-to-date
    /// view of project activity across the system.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `visibility_filter` - Optional visibility filter (Public, Private, etc.)
    /// * `status_filter` - Optional status filter (Active, Archived, etc.)
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Project` entries matching the filter criteria,
    /// ordered by update time (most recent first), or a database error if the query fails.
    ///
    /// # Administrative Use Cases
    ///
    /// - System-wide project listings and dashboards
    /// - Project discovery and browsing interfaces
    /// - Administrative oversight and monitoring
    /// - Analytics and reporting on project activity
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

    /// Searches projects by name or description using text matching.
    ///
    /// Performs a case-insensitive text search across project names and
    /// descriptions to find matching projects. Only searches public projects
    /// to respect privacy settings and provides relevant results for
    /// project discovery and exploration.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `search_query` - Text to search for in project names and descriptions
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of public `Project` entries matching the search criteria,
    /// ordered by update time (most recent first), or a database error if the query fails.
    ///
    /// # Search Features
    ///
    /// - Case-insensitive partial matching
    /// - Searches both project names and descriptions
    /// - Respects project visibility settings (public only)
    /// - Orders results by recent activity
    ///
    /// # Discovery Use Cases
    ///
    /// - Public project discovery and exploration
    /// - Finding projects by topic or keyword
    /// - Building project recommendation systems
    /// - Community project browsing interfaces
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

    /// Finds projects by tags using array overlap matching.
    ///
    /// Searches for projects that have at least one tag in common with
    /// the provided tag list. This enables topic-based project discovery
    /// and categorical browsing. Excludes suspended projects to ensure
    /// quality results.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `search_tags` - Array of tags to search for (matches any)
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Project` entries with overlapping tags, ordered by
    /// update time (most recent first), or a database error if the query fails.
    ///
    /// # Tag Matching Logic
    ///
    /// - Uses PostgreSQL array overlap operator for efficient matching
    /// - Finds projects with ANY of the specified tags
    /// - Excludes suspended projects for quality assurance
    /// - Orders by recent activity for relevance
    ///
    /// # Discovery and Categorization Use Cases
    ///
    /// - Topic-based project discovery
    /// - Technology stack filtering
    /// - Category-specific project browsing
    /// - Building tag-based recommendation systems
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

    /// Retrieves basic project statistics including members, invites, and activity.
    ///
    /// Compiles essential metrics about a project's current state including
    /// active membership count, pending invitation count, and activity levels.
    /// These statistics are fundamental for project dashboards and
    /// administrative oversight.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the queries
    /// * `project_id` - UUID of the project to get statistics for
    ///
    /// # Returns
    ///
    /// A tuple containing `(member_count, pending_invites, activity_count)`,
    /// or a database error if any of the queries fail.
    ///
    /// # Statistics Components
    ///
    /// - `member_count`: Number of active project members
    /// - `pending_invites`: Number of outstanding invitation requests
    /// - `activity_count`: Total activity metrics (currently placeholder)
    ///
    /// # Dashboard and Monitoring Use Cases
    ///
    /// - Project overview dashboards
    /// - Administrative project monitoring
    /// - Member engagement analytics
    /// - Project health assessments
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
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count total activity (placeholder for now)
        let activity_count: i64 = 0; // Would need to implement activity counting

        Ok((member_count, pending_invites, activity_count))
    }

    /// Gets the total count of projects created by a specific user.
    ///
    /// Returns a simple count of all non-deleted projects where the specified
    /// user is the creator. This metric is useful for user profiles,
    /// administrative reporting, and understanding user engagement with
    /// the platform.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `user_id` - UUID of the user to count projects for
    ///
    /// # Returns
    ///
    /// The total count of projects created by the user,
    /// or a database error if the query fails.
    ///
    /// # User Analytics Use Cases
    ///
    /// - User profile statistics and achievements
    /// - Administrative user activity monitoring
    /// - Platform engagement analytics
    /// - User contribution metrics
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
}
