//! Project activity repository for managing project activity log operations.
//!
//! This module provides database query operations for project activity logs,
//! enabling comprehensive tracking and analysis of all project-related activities.
//! It handles creation, retrieval, filtering, and analysis of activity data with
//! support for various query patterns and reporting needs.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectActivity, ProjectActivity};
use crate::types::ActivityType;
use crate::{PgError, PgResult, schema};

/// Parameters for logging entity-specific activities with full context capture.
///
/// This structure encapsulates all the information needed to create comprehensive
/// activity log entries for various types of project activities. It provides a
/// standardized way to capture user actions, system events, and security-relevant
/// information across different entity types (documents, members, integrations, etc.).
///
/// The parameters support both user-initiated and system-generated activities,
/// with optional fields for scenarios where complete information isn't available.
#[derive(Debug, Clone)]
pub struct LogEntityActivityParams {
    /// The account that performed or initiated the activity.
    ///
    /// Foreign key to the account responsible for this activity. Set to `None`
    /// for system-generated activities, background processes, or anonymous actions.
    /// Used for attribution and user activity tracking.
    pub account_id: Option<Uuid>,

    /// The specific type of activity being logged.
    ///
    /// Categorizes the activity using predefined types that determine how the
    /// activity is processed, displayed, and analyzed. Must correspond to the
    /// actual operation being performed.
    pub activity_type: ActivityType,

    /// Human-readable description of what occurred.
    ///
    /// Detailed explanation of the activity that provides context and specifics
    /// about the operation. Should be clear enough for audit purposes and
    /// activity feeds (recommended max 1000 characters).
    pub description: String,

    /// Structured metadata containing activity-specific details.
    ///
    /// JSON object with additional information relevant to this activity type,
    /// such as entity IDs, before/after values, configuration changes, or
    /// other contextual data needed for analysis and audit trails.
    pub metadata: serde_json::Value,

    /// Network address of the client that initiated the activity.
    ///
    /// IP address used for security monitoring, geographic analysis, and
    /// detecting suspicious activity patterns. Captured from the request
    /// when available.
    pub ip_address: Option<IpNet>,

    /// User agent string from the client request.
    ///
    /// HTTP User-Agent header identifying the browser, mobile app, or API client
    /// used to perform the activity. Useful for usage pattern analysis and
    /// security monitoring.
    pub user_agent: Option<String>,
}

/// Repository for project activity log table operations.
///
/// Provides comprehensive database operations for managing project activity logs,
/// including activity creation, querying, analysis, and maintenance. This repository
/// handles all database interactions related to activity tracking and audit trails.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectActivityRepository;

impl ProjectActivityRepository {
    /// Creates a new project activity repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `ProjectActivityRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Logs a new activity in the project activity log.
    ///
    /// Creates a new activity log entry with the provided information. This is the
    /// primary method for recording all project-related activities, whether user-initiated
    /// or system-generated. The activity is immediately persisted to the database
    /// and returned with its assigned ID and creation timestamp.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `activity` - Complete activity data to be logged
    ///
    /// # Returns
    ///
    /// The created `ProjectActivity` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
    pub async fn log_activity(
        conn: &mut AsyncPgConnection,
        activity: NewProjectActivity,
    ) -> PgResult<ProjectActivity> {
        use schema::project_activities;

        let activity = diesel::insert_into(project_activities::table)
            .values(&activity)
            .returning(ProjectActivity::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activity)
    }

    /// Lists activities for a specific project with pagination support.
    ///
    /// Retrieves project activities in reverse chronological order (most recent first)
    /// with configurable pagination. This is the primary method for building activity
    /// feeds and audit trails for individual projects.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project whose activities to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectActivity` entries ordered by creation time (newest first),
    /// or a database error if the query fails.
    pub async fn list_project_activity(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets recent activities across all projects for a specific user.
    ///
    /// Retrieves activities performed by a specific user across all projects they
    /// have access to, ordered by recency. This is useful for building user-specific
    /// activity dashboards and tracking individual user engagement.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `user_id` - UUID of the user whose activities to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectActivity` entries performed by the user, ordered by
    /// creation time (newest first), or a database error if the query fails.
    pub async fn get_user_recent_activity(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let activities = project_activities
            .filter(account_id.eq(user_id))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets activities of a specific type within a project.
    ///
    /// Filters project activities to only include entries of the specified activity
    /// type. This is useful for analyzing specific types of actions, building
    /// specialized activity feeds, or investigating particular categories of events.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to search within
    /// * `activity_type_filter` - Specific activity type to filter by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectActivity` entries matching the specified type, ordered by
    /// creation time (newest first), or a database error if the query fails.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get all document creation activities for a project
    /// let activities = ProjectActivityRepository::get_activity_by_type(
    ///     &mut conn,
    ///     project_id,
    ///     ActivityType::DocumentCreated,
    ///     Pagination::default()
    /// ).await?;
    /// ```
    pub async fn get_activity_by_type(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        activity_type_filter: ActivityType,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .filter(activity_type.eq(activity_type_filter))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets recent activities for a user within a specified time window.
    ///
    /// Retrieves activities performed by a specific user within the last N hours
    /// across all projects. This provides a time-bounded view of user activity,
    /// useful for recent activity summaries and engagement tracking.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `user_id` - UUID of the user whose recent activities to retrieve
    /// * `hours` - Number of hours back to search for activities
    ///
    /// # Returns
    ///
    /// A vector of up to 50 `ProjectActivity` entries within the time window,
    /// ordered by creation time (newest first), or a database error if the query fails.
    pub async fn get_recent_user_activity(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
        hours: i64,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(hours);

        let activities = project_activities
            .filter(account_id.eq(user_id))
            .filter(created_at.gt(cutoff_time))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(50)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets all activities for a project with pagination support.
    ///
    /// This method is functionally identical to `list_project_activity` and provides
    /// an alternative naming convention. Retrieves all activities for a specific
    /// project in reverse chronological order with pagination support.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project whose activities to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectActivity` entries ordered by creation time (newest first),
    /// or a database error if the query fails.
    ///
    /// # Note
    ///
    /// Consider using `list_project_activity` for consistency with repository naming
    /// conventions, as both methods provide identical functionality.
    pub async fn get_activities_by_project(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Logs integration-related activity using standardized parameters.
    ///
    /// Convenience method for logging activities related to project integrations
    /// using the standardized parameter structure. This ensures consistent activity
    /// logging across different integration operations and entity types.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project_id` - UUID of the project the integration belongs to
    /// * `params` - Standardized activity parameters with all context information
    ///
    /// # Returns
    ///
    /// The created `ProjectActivity` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
    pub async fn log_integration_activity(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<ProjectActivity> {
        let activity = NewProjectActivity {
            project_id,
            account_id: params.account_id,
            activity_type: params.activity_type,
            description: Some(params.description),
            metadata: Some(params.metadata),
            ip_address: params.ip_address,
            user_agent: params.user_agent,
        };

        Self::log_activity(conn, activity).await
    }

    /// Logs project member-related activity using standardized parameters.
    ///
    /// Convenience method for logging activities related to project membership
    /// changes, such as member additions, removals, role updates, or permission
    /// modifications. Uses the standardized parameter structure for consistency.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project_id` - UUID of the project where member activity occurred
    /// * `params` - Standardized activity parameters with all context information
    ///
    /// # Returns
    ///
    /// The created `ProjectActivity` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
    pub async fn log_member_activity(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<ProjectActivity> {
        let activity = NewProjectActivity {
            project_id,
            account_id: params.account_id,
            activity_type: params.activity_type,
            description: Some(params.description),
            metadata: Some(params.metadata),
            ip_address: params.ip_address,
            user_agent: params.user_agent,
        };

        Self::log_activity(conn, activity).await
    }

    /// Logs document-related activity using standardized parameters.
    ///
    /// Convenience method for logging activities related to document operations
    /// such as creation, updates, deletions, comments, or version changes.
    /// Uses the standardized parameter structure for consistent activity tracking.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `project_id` - UUID of the project containing the document
    /// * `params` - Standardized activity parameters with all context information
    ///
    /// # Returns
    ///
    /// The created `ProjectActivity` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
    pub async fn log_document_activity(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<ProjectActivity> {
        let activity = NewProjectActivity {
            project_id,
            account_id: params.account_id,
            activity_type: params.activity_type,
            description: Some(params.description),
            metadata: Some(params.metadata),
            ip_address: params.ip_address,
            user_agent: params.user_agent,
        };

        Self::log_activity(conn, activity).await
    }

    /// Gets activity count statistics for a project within an optional time window.
    ///
    /// Calculates the total number of activities for a project, optionally filtered
    /// to only include activities within the specified number of hours. This provides
    /// basic activity metrics for dashboard displays and project health monitoring.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to analyze
    /// * `hours` - Optional time window in hours (None for all-time statistics)
    ///
    /// # Returns
    ///
    /// Total count of activities matching the criteria, or a database error
    /// if the query fails.
    pub async fn get_activity_stats(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        hours: Option<i64>,
    ) -> PgResult<i64> {
        use schema::project_activities::dsl::*;

        let mut query = project_activities
            .filter(project_id.eq(proj_id))
            .into_boxed();

        if let Some(time_window) = hours {
            let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(time_window);
            query = query.filter(created_at.gt(cutoff_time));
        }

        let count: i64 = query
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    /// Gets the most active users in a project ranked by activity count.
    ///
    /// Analyzes project activities to identify users with the highest activity levels,
    /// optionally within a specified time window. This is useful for recognizing
    /// engaged users, identifying project contributors, and understanding usage patterns.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to analyze
    /// * `hours` - Optional time window in hours (None for all-time analysis)
    /// * `limit` - Maximum number of users to return in the ranking
    ///
    /// # Returns
    ///
    /// A vector of tuples containing user IDs and their activity counts, ordered by
    /// activity count (highest first). System activities (None user ID) are excluded.
    /// Returns a database error if the query fails.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Get top 10 most active users in the last week
    /// let top_users = ProjectActivityRepository::get_most_active_users(
    ///     &mut conn, project_id, Some(168), 10  // 168 hours = 7 days
    /// ).await?;
    ///
    /// for (user_id, activity_count) in top_users {
    ///     if let Some(uid) = user_id {
    ///         println!("User {} has {} activities", uid, activity_count);
    ///     }
    /// }
    /// ```
    pub async fn get_most_active_users(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        hours: Option<i64>,
        limit: i64,
    ) -> PgResult<Vec<(Option<Uuid>, i64)>> {
        use schema::project_activities::dsl::*;

        let results = if let Some(time_window) = hours {
            let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(time_window);
            project_activities
                .filter(project_id.eq(proj_id))
                .filter(account_id.is_not_null())
                .filter(created_at.gt(cutoff_time))
                .group_by(account_id)
                .select((account_id, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .limit(limit)
                .load::<(Option<Uuid>, i64)>(conn)
                .await
                .map_err(PgError::from)?
        } else {
            project_activities
                .filter(project_id.eq(proj_id))
                .filter(account_id.is_not_null())
                .group_by(account_id)
                .select((account_id, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .limit(limit)
                .load::<(Option<Uuid>, i64)>(conn)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    /// Gets a breakdown of activities by type for analytical reporting.
    ///
    /// Analyzes project activities to provide a count-based breakdown by activity type,
    /// optionally within a specified time window. This helps understand what types of
    /// activities are most common and provides insights into project usage patterns.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to analyze
    /// * `hours` - Optional time window in hours (None for all-time analysis)
    ///
    /// # Returns
    ///
    /// A vector of tuples containing activity types and their occurrence counts,
    /// ordered by count (highest first). Returns a database error if the query fails.
    pub async fn get_activity_type_breakdown(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        hours: Option<i64>,
    ) -> PgResult<Vec<(ActivityType, i64)>> {
        use schema::project_activities::dsl::*;

        let results = if let Some(time_window) = hours {
            let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(time_window);
            project_activities
                .filter(project_id.eq(proj_id))
                .filter(created_at.gt(cutoff_time))
                .group_by(activity_type)
                .select((activity_type, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .load::<(ActivityType, i64)>(conn)
                .await
                .map_err(PgError::from)?
        } else {
            project_activities
                .filter(project_id.eq(proj_id))
                .group_by(activity_type)
                .select((activity_type, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .load::<(ActivityType, i64)>(conn)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    /// Gets system-generated activities that have no associated user account.
    ///
    /// Retrieves activities that were generated by automated processes, background
    /// jobs, integrations, or other system operations rather than direct user actions.
    /// These activities provide insight into automated system behavior and integration
    /// operations within the project.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to search within
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectActivity` entries with no associated account ID, ordered by
    /// creation time (newest first), or a database error if the query fails.
    pub async fn get_system_activities(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .filter(account_id.is_null())
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets activities originating from a specific IP address for security analysis.
    ///
    /// Retrieves all activities within a project that originated from the specified
    /// IP address. This is useful for security investigations, detecting suspicious
    /// activity patterns, or analyzing access from particular network locations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to search within
    /// * `ip_addr` - Specific IP address to filter activities by
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectActivity` entries from the specified IP address, ordered by
    /// creation time (newest first), or a database error if the query fails.
    ///
    /// # Security Use Cases
    ///
    /// - Investigating suspicious activity from unknown IP addresses
    /// - Tracking activities from specific geographic locations
    /// - Monitoring access patterns from corporate networks
    /// - Forensic analysis of security incidents
    pub async fn get_activities_by_ip(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        ip_addr: IpNet,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .filter(ip_address.eq(ip_addr))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Cleans up old activity logs to manage database size and performance.
    ///
    /// Permanently deletes activity log entries older than the specified number of days.
    /// This maintenance operation helps control database growth and maintain optimal
    /// query performance by removing historical data that is no longer needed.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `days_to_keep` - Number of days of activity history to preserve
    ///
    /// # Returns
    ///
    /// The number of activity records that were deleted, or a database error
    /// if the operation fails.
    ///
    /// # Caution
    ///
    /// This operation permanently deletes data and cannot be undone. Ensure
    /// you have appropriate backups if the deleted activity data may be needed
    /// for compliance or audit purposes.
    pub async fn cleanup_old_activities(
        conn: &mut AsyncPgConnection,
        days_to_keep: i64,
    ) -> PgResult<usize> {
        use schema::project_activities::dsl::*;

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(days_to_keep);

        let deleted_count = diesel::delete(project_activities)
            .filter(created_at.lt(cutoff_date))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(deleted_count)
    }
}
