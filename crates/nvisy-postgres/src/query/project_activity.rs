//! Project activity repository for managing project activity log operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectActivity, ProjectActivity};
use crate::{PgError, PgResult, schema};

/// Parameters for logging entity-specific activities.
#[derive(Debug, Clone)]
pub struct LogEntityActivityParams {
    /// The entity ID (integration, member, or document)
    pub entity_id: Uuid,
    /// The actor performing the activity
    pub actor_id: Option<Uuid>,
    /// The type of activity
    pub activity_type: String,
    /// Additional activity data
    pub activity_data: serde_json::Value,
    /// IP address of the actor
    pub ip_address: Option<IpNet>,
    /// User agent of the actor
    pub user_agent: Option<String>,
}

/// Repository for project activity log table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectActivityRepository;

impl ProjectActivityRepository {
    /// Creates a new project activity repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Logs a new activity in the project activity log.
    pub async fn log_activity(
        conn: &mut AsyncPgConnection,
        activity: NewProjectActivity,
    ) -> PgResult<ProjectActivity> {
        use schema::project_activity_log;

        let activity = diesel::insert_into(project_activity_log::table)
            .values(&activity)
            .returning(ProjectActivity::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activity)
    }

    /// Lists activity for a specific project.
    pub async fn list_project_activity(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activity_log::dsl::*;

        let activities = project_activity_log
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

    /// Gets recent activity across projects for a specific user.
    pub async fn get_user_recent_activity(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activity_log::dsl::*;

        let activities = project_activity_log
            .filter(actor_id.eq(user_id))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets activity by type for a project.
    pub async fn get_activity_by_type(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        activity_type_filter: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activity_log::dsl::*;

        let activities = project_activity_log
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

    /// Gets recent activity across all projects for a user.
    pub async fn get_recent_user_activity(
        conn: &mut AsyncPgConnection,
        user_id: Uuid,
        hours: i64,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activity_log::dsl::*;

        let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(hours);

        let activities = project_activity_log
            .filter(actor_id.eq(user_id))
            .filter(created_at.gt(cutoff_time))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(50)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets activity for a specific entity within a project.
    pub async fn get_entity_activity(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        entity_type_filter: &str,
        entity_id_filter: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activity_log::dsl::*;

        let activities = project_activity_log
            .filter(project_id.eq(proj_id))
            .filter(entity_type.eq(entity_type_filter))
            .filter(entity_id.eq(entity_id_filter))
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Logs integration activity using the project activity log.
    pub async fn log_integration_activity(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<ProjectActivity> {
        let activity = NewProjectActivity {
            project_id,
            actor_id: params.actor_id,
            activity_type: params.activity_type,
            activity_data: params.activity_data,
            entity_type: Some("integration".to_string()),
            entity_id: Some(params.entity_id),
            ip_address: params.ip_address,
            user_agent: params.user_agent,
        };

        Self::log_activity(conn, activity).await
    }

    /// Logs member activity using the project activity log.
    pub async fn log_member_activity(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<ProjectActivity> {
        let activity = NewProjectActivity {
            project_id,
            actor_id: params.actor_id,
            activity_type: params.activity_type,
            activity_data: params.activity_data,
            entity_type: Some("member".to_string()),
            entity_id: Some(params.entity_id),
            ip_address: params.ip_address,
            user_agent: params.user_agent,
        };

        Self::log_activity(conn, activity).await
    }

    /// Logs document activity using the project activity log.
    pub async fn log_document_activity(
        conn: &mut AsyncPgConnection,
        project_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<ProjectActivity> {
        let activity = NewProjectActivity {
            project_id,
            actor_id: params.actor_id,
            activity_type: params.activity_type,
            activity_data: params.activity_data,
            entity_type: Some("document".to_string()),
            entity_id: Some(params.entity_id),
            ip_address: params.ip_address,
            user_agent: params.user_agent,
        };

        Self::log_activity(conn, activity).await
    }

    /// Gets activity statistics for a project.
    pub async fn get_activity_stats(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        hours: Option<i64>,
    ) -> PgResult<i64> {
        use schema::project_activity_log::dsl::*;

        let mut query = project_activity_log
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

    /// Gets the most active users in a project.
    pub async fn get_most_active_users(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        hours: Option<i64>,
        limit: i64,
    ) -> PgResult<Vec<(Option<Uuid>, i64)>> {
        use schema::project_activity_log::dsl::*;

        let results = if let Some(time_window) = hours {
            let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(time_window);
            project_activity_log
                .filter(project_id.eq(proj_id))
                .filter(actor_id.is_not_null())
                .filter(created_at.gt(cutoff_time))
                .group_by(actor_id)
                .select((actor_id, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .limit(limit)
                .load::<(Option<Uuid>, i64)>(conn)
                .await
                .map_err(PgError::from)?
        } else {
            project_activity_log
                .filter(project_id.eq(proj_id))
                .filter(actor_id.is_not_null())
                .group_by(actor_id)
                .select((actor_id, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .limit(limit)
                .load::<(Option<Uuid>, i64)>(conn)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    /// Gets activity breakdown by type for a project.
    pub async fn get_activity_type_breakdown(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        hours: Option<i64>,
    ) -> PgResult<Vec<(String, i64)>> {
        use schema::project_activity_log::dsl::*;

        let results = if let Some(time_window) = hours {
            let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(time_window);
            project_activity_log
                .filter(project_id.eq(proj_id))
                .filter(created_at.gt(cutoff_time))
                .group_by(activity_type)
                .select((activity_type, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .load::<(String, i64)>(conn)
                .await
                .map_err(PgError::from)?
        } else {
            project_activity_log
                .filter(project_id.eq(proj_id))
                .group_by(activity_type)
                .select((activity_type, diesel::dsl::count(id)))
                .order(diesel::dsl::count(id).desc())
                .load::<(String, i64)>(conn)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    /// Gets system-generated activities (no actor).
    pub async fn get_system_activities(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activity_log::dsl::*;

        let activities = project_activity_log
            .filter(project_id.eq(proj_id))
            .filter(actor_id.is_null())
            .select(ProjectActivity::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    /// Gets activities from a specific IP address.
    pub async fn get_activities_by_ip(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        ip_addr: IpNet,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectActivity>> {
        use schema::project_activity_log::dsl::*;

        let activities = project_activity_log
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

    /// Cleans up old activity logs (older than specified days).
    pub async fn cleanup_old_activities(
        conn: &mut AsyncPgConnection,
        days_to_keep: i64,
    ) -> PgResult<usize> {
        use schema::project_activity_log::dsl::*;

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(days_to_keep);

        let deleted_count = diesel::delete(project_activity_log)
            .filter(created_at.lt(cutoff_date))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(deleted_count)
    }
}
