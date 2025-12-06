//! Project activity repository for managing project activity log operations.
//!
//! This module provides database query operations for project activity logs,
//! enabling comprehensive tracking and analysis of all project-related activities.
//! It handles creation, retrieval, filtering, and analysis of activity data with
//! support for various query patterns and reporting needs.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectActivity, ProjectActivity};
use crate::types::ActivityType;
use crate::{PgClient, PgError, PgResult, schema};

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
    pub account_id: Option<Uuid>,

    /// The specific type of activity being logged.
    pub activity_type: ActivityType,

    /// Human-readable description of what occurred.
    pub description: String,

    /// Structured metadata containing activity-specific details.
    pub metadata: serde_json::Value,

    /// Network address of the client that initiated the activity.
    pub ip_address: Option<IpNet>,

    /// User agent string from the client request.
    pub user_agent: Option<String>,
}

/// Repository for project activity log table operations.
///
/// Provides comprehensive database operations for managing project activity logs,
/// including activity creation, querying, analysis, and maintenance. This repository
/// handles all database interactions related to activity tracking and audit trails.
pub trait ProjectActivityRepository {
    /// Logs a new activity in the project activity log.
    fn log_activity(
        &self,
        activity: NewProjectActivity,
    ) -> impl Future<Output = PgResult<ProjectActivity>> + Send;

    /// Lists activities for a specific project with pagination support.
    fn list_project_activity(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectActivity>>> + Send;

    /// Gets recent activities across all projects for a specific user.
    fn get_user_recent_activity(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectActivity>>> + Send;

    /// Gets activities of a specific type within a project.
    fn get_activity_by_type(
        &self,
        proj_id: Uuid,
        activity_type_filter: ActivityType,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectActivity>>> + Send;

    /// Gets recent activities for a user within a specified time window.
    fn get_recent_user_activity(
        &self,
        user_id: Uuid,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<ProjectActivity>>> + Send;

    /// Gets all activities for a project with pagination support.
    fn get_activities_by_project(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectActivity>>> + Send;

    /// Logs integration-related activity using standardized parameters.
    fn log_integration_activity(
        &self,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> impl Future<Output = PgResult<ProjectActivity>> + Send;

    /// Logs project member-related activity using standardized parameters.
    fn log_member_activity(
        &self,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> impl Future<Output = PgResult<ProjectActivity>> + Send;

    /// Logs document-related activity using standardized parameters.
    fn log_document_activity(
        &self,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> impl Future<Output = PgResult<ProjectActivity>> + Send;

    /// Gets activity count statistics for a project within an optional time window.
    fn get_activity_stats(
        &self,
        proj_id: Uuid,
        hours: Option<i64>,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Gets the most active users in a project ranked by activity count.
    fn get_most_active_users(
        &self,
        proj_id: Uuid,
        hours: Option<i64>,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<(Option<Uuid>, i64)>>> + Send;

    /// Gets a breakdown of activities by type for analytical reporting.
    fn get_activity_type_breakdown(
        &self,
        proj_id: Uuid,
        hours: Option<i64>,
    ) -> impl Future<Output = PgResult<Vec<(ActivityType, i64)>>> + Send;

    /// Gets system-generated activities that have no associated user account.
    fn get_system_activities(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectActivity>>> + Send;

    /// Gets activities originating from a specific IP address for security analysis.
    fn get_activities_by_ip(
        &self,
        proj_id: Uuid,
        ip_addr: IpNet,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectActivity>>> + Send;

    /// Cleans up old activity logs to manage database size and performance.
    fn cleanup_old_activities(
        &self,
        days_to_keep: i64,
    ) -> impl Future<Output = PgResult<usize>> + Send;
}

impl ProjectActivityRepository for PgClient {
    async fn log_activity(&self, activity: NewProjectActivity) -> PgResult<ProjectActivity> {
        use schema::project_activities;

        let mut conn = self.get_connection().await?;

        let activity = diesel::insert_into(project_activities::table)
            .values(&activity)
            .returning(ProjectActivity::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activity)
    }

    async fn list_project_activity(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_user_recent_activity(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let activities = project_activities
            .filter(account_id.eq(user_id))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_activity_by_type(
        &self,
        proj_id: Uuid,
        activity_type_filter: ActivityType,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .filter(activity_type.eq(activity_type_filter))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_recent_user_activity(
        &self,
        user_id: Uuid,
        hours: i64,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(hours);

        let activities = project_activities
            .filter(account_id.eq(user_id))
            .filter(created_at.gt(cutoff_time))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(50)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_activities_by_project(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn log_integration_activity(
        &self,
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

        self.log_activity(activity).await
    }

    async fn log_member_activity(
        &self,
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

        self.log_activity(activity).await
    }

    async fn log_document_activity(
        &self,
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

        self.log_activity(activity).await
    }

    async fn get_activity_stats(&self, proj_id: Uuid, hours: Option<i64>) -> PgResult<i64> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let mut query = project_activities
            .filter(project_id.eq(proj_id))
            .into_boxed();

        if let Some(time_window) = hours {
            let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(time_window);
            query = query.filter(created_at.gt(cutoff_time));
        }

        let count: i64 = query
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn get_most_active_users(
        &self,
        proj_id: Uuid,
        hours: Option<i64>,
        limit: i64,
    ) -> PgResult<Vec<(Option<Uuid>, i64)>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

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
                .load::<(Option<Uuid>, i64)>(&mut conn)
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
                .load::<(Option<Uuid>, i64)>(&mut conn)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    async fn get_activity_type_breakdown(
        &self,
        proj_id: Uuid,
        hours: Option<i64>,
    ) -> PgResult<Vec<(ActivityType, i64)>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let results = if let Some(time_window) = hours {
            let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(time_window);
            project_activities
                .filter(project_id.eq(proj_id))
                .filter(created_at.gt(cutoff_time))
                .group_by(activity_type)
                .select((activity_type, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .load::<(ActivityType, i64)>(&mut conn)
                .await
                .map_err(PgError::from)?
        } else {
            project_activities
                .filter(project_id.eq(proj_id))
                .group_by(activity_type)
                .select((activity_type, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .load::<(ActivityType, i64)>(&mut conn)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    async fn get_system_activities(
        &self,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .filter(account_id.is_null())
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_activities_by_ip(
        &self,
        proj_id: Uuid,
        ip_addr: IpNet,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let activities = project_activities
            .filter(project_id.eq(proj_id))
            .filter(ip_address.eq(ip_addr))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn cleanup_old_activities(&self, days_to_keep: i64) -> PgResult<usize> {
        use schema::project_activities::dsl::*;

        let mut conn = self.get_connection().await?;

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(days_to_keep);

        let deleted_count = diesel::delete(project_activities)
            .filter(created_at.lt(cutoff_date))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(deleted_count)
    }
}
