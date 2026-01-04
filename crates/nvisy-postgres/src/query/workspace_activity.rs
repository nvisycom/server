//! Workspace activity repository for managing workspace activity log operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use ipnet::IpNet;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use crate::model::{NewWorkspaceActivity, WorkspaceActivity};
use crate::types::{ActivityType, CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Parameters for logging entity-specific activities.
#[derive(Debug, Clone)]
pub struct LogEntityActivityParams {
    /// The account that performed the activity.
    pub account_id: Option<Uuid>,
    /// The type of activity being logged.
    pub activity_type: ActivityType,
    /// Human-readable description.
    pub description: String,
    /// Structured metadata with activity details.
    pub metadata: serde_json::Value,
    /// Client IP address.
    pub ip_address: Option<IpNet>,
    /// Client user agent string.
    pub user_agent: Option<String>,
}

/// Repository for workspace activity log database operations.
///
/// Handles activity logging, querying, and audit trail management.
pub trait WorkspaceActivityRepository {
    /// Logs a new activity in the workspace activity log.
    fn log_activity(
        &mut self,
        activity: NewWorkspaceActivity,
    ) -> impl Future<Output = PgResult<WorkspaceActivity>> + Send;

    /// Lists activities for a specific workspace with offset pagination.
    fn offset_list_workspace_activity(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceActivity>>> + Send;

    /// Lists activities for a specific workspace with cursor pagination.
    fn cursor_list_workspace_activity(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceActivity>>> + Send;

    /// Gets recent activities across all workspaces for a specific user.
    fn get_account_recent_activity(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceActivity>>> + Send;

    /// Gets activities of a specific type within a workspace.
    fn get_activity_by_type(
        &mut self,
        workspace_id: Uuid,
        activity_type_filter: ActivityType,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceActivity>>> + Send;

    /// Gets recent activities for a user within a specified time window.
    fn get_recent_account_activity(
        &mut self,
        account_id: Uuid,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceActivity>>> + Send;

    /// Logs integration-related activity using standardized parameters.
    fn log_integration_activity(
        &mut self,
        workspace_id: Uuid,
        params: LogEntityActivityParams,
    ) -> impl Future<Output = PgResult<WorkspaceActivity>> + Send;

    /// Logs workspace member-related activity using standardized parameters.
    fn log_member_activity(
        &mut self,
        workspace_id: Uuid,
        params: LogEntityActivityParams,
    ) -> impl Future<Output = PgResult<WorkspaceActivity>> + Send;

    /// Logs document-related activity using standardized parameters.
    fn log_document_activity(
        &mut self,
        workspace_id: Uuid,
        params: LogEntityActivityParams,
    ) -> impl Future<Output = PgResult<WorkspaceActivity>> + Send;

    /// Gets the most active users in a workspace ranked by activity count.
    fn get_most_active_accounts(
        &mut self,
        workspace_id: Uuid,
        hours: Option<i64>,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<(Option<Uuid>, i64)>>> + Send;

    /// Gets a breakdown of activities by type for analytical reporting.
    fn get_activity_type_breakdown(
        &mut self,
        workspace_id: Uuid,
        hours: Option<i64>,
    ) -> impl Future<Output = PgResult<Vec<(ActivityType, i64)>>> + Send;

    /// Gets system-generated activities that have no associated user account.
    fn get_system_activities(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceActivity>>> + Send;

    /// Gets activities originating from a specific IP address for security analysis.
    fn get_activities_by_ip(
        &mut self,
        workspace_id: Uuid,
        ip_addr: IpNet,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceActivity>>> + Send;

    /// Cleans up old activity logs to manage database size and performance.
    fn cleanup_old_activities(
        &mut self,
        days_to_keep: i64,
    ) -> impl Future<Output = PgResult<usize>> + Send;
}

impl WorkspaceActivityRepository for PgConnection {
    async fn log_activity(
        &mut self,
        activity: NewWorkspaceActivity,
    ) -> PgResult<WorkspaceActivity> {
        use schema::workspace_activities;

        let activity = diesel::insert_into(workspace_activities::table)
            .values(&activity)
            .returning(WorkspaceActivity::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(activity)
    }

    async fn offset_list_workspace_activity(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceActivity>> {
        use schema::workspace_activities::{self, dsl};

        let activities = workspace_activities::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .select(WorkspaceActivity::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn cursor_list_workspace_activity(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceActivity>> {
        use diesel::dsl::count_star;
        use schema::workspace_activities::{self, dsl};

        // Get total count only if requested
        let total = if pagination.include_count {
            Some(
                workspace_activities::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Build query with cursor
        let mut query = workspace_activities::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .into_boxed();

        if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query = query.filter(
                dsl::created_at
                    .lt(cursor_ts)
                    .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
            );
        }

        let items: Vec<WorkspaceActivity> = query
            .select(WorkspaceActivity::as_select())
            .order((dsl::created_at.desc(), dsl::id.desc()))
            .limit(pagination.fetch_limit())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(CursorPage::new(items, total, pagination.limit, |a| {
            (a.created_at.into(), a.id)
        }))
    }

    async fn get_account_recent_activity(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceActivity>> {
        use schema::workspace_activities::{self, dsl};

        let activities = workspace_activities::table
            .filter(dsl::account_id.eq(account_id))
            .select(WorkspaceActivity::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_activity_by_type(
        &mut self,
        workspace_id: Uuid,
        activity_type_filter: ActivityType,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceActivity>> {
        use schema::workspace_activities::{self, dsl};

        let activities = workspace_activities::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::activity_type.eq(activity_type_filter))
            .select(WorkspaceActivity::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_recent_account_activity(
        &mut self,
        account_id: Uuid,
        hours: i64,
    ) -> PgResult<Vec<WorkspaceActivity>> {
        use schema::workspace_activities::{self, dsl};

        let cutoff_time = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(hours));

        let activities = workspace_activities::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::created_at.gt(cutoff_time))
            .select(WorkspaceActivity::as_select())
            .order(dsl::created_at.desc())
            .limit(50)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn log_integration_activity(
        &mut self,
        workspace_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<WorkspaceActivity> {
        let activity = NewWorkspaceActivity {
            workspace_id,
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
        &mut self,
        workspace_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<WorkspaceActivity> {
        let activity = NewWorkspaceActivity {
            workspace_id,
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
        &mut self,
        workspace_id: Uuid,
        params: LogEntityActivityParams,
    ) -> PgResult<WorkspaceActivity> {
        let activity = NewWorkspaceActivity {
            workspace_id,
            account_id: params.account_id,
            activity_type: params.activity_type,
            description: Some(params.description),
            metadata: Some(params.metadata),
            ip_address: params.ip_address,
            user_agent: params.user_agent,
        };

        self.log_activity(activity).await
    }

    async fn get_most_active_accounts(
        &mut self,
        workspace_id: Uuid,
        hours: Option<i64>,
        limit: i64,
    ) -> PgResult<Vec<(Option<Uuid>, i64)>> {
        use schema::workspace_activities::{self, dsl};

        let results = if let Some(time_window) = hours {
            let cutoff_time =
                jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(time_window));
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::account_id.is_not_null())
                .filter(dsl::created_at.gt(cutoff_time))
                .group_by(dsl::account_id)
                .select((dsl::account_id, diesel::dsl::count(dsl::id)))
                .order(diesel::dsl::count(dsl::id).desc())
                .limit(limit)
                .load::<(Option<Uuid>, i64)>(self)
                .await
                .map_err(PgError::from)?
        } else {
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::account_id.is_not_null())
                .group_by(dsl::account_id)
                .select((dsl::account_id, diesel::dsl::count(dsl::id)))
                .order(diesel::dsl::count(dsl::id).desc())
                .limit(limit)
                .load::<(Option<Uuid>, i64)>(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    async fn get_activity_type_breakdown(
        &mut self,
        workspace_id: Uuid,
        hours: Option<i64>,
    ) -> PgResult<Vec<(ActivityType, i64)>> {
        use schema::workspace_activities::{self, dsl};

        let results = if let Some(time_window) = hours {
            let cutoff_time =
                jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(time_window));
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::created_at.gt(cutoff_time))
                .group_by(dsl::activity_type)
                .select((dsl::activity_type, diesel::dsl::count(dsl::id)))
                .order(diesel::dsl::count(dsl::id).desc())
                .load::<(ActivityType, i64)>(self)
                .await
                .map_err(PgError::from)?
        } else {
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .group_by(dsl::activity_type)
                .select((dsl::activity_type, diesel::dsl::count(dsl::id)))
                .order(diesel::dsl::count(dsl::id).desc())
                .load::<(ActivityType, i64)>(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(results)
    }

    async fn get_system_activities(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceActivity>> {
        use schema::workspace_activities::{self, dsl};

        let activities = workspace_activities::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.is_null())
            .select(WorkspaceActivity::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn get_activities_by_ip(
        &mut self,
        workspace_id: Uuid,
        ip_addr: IpNet,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceActivity>> {
        use schema::workspace_activities::{self, dsl};

        let activities = workspace_activities::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::ip_address.eq(ip_addr))
            .select(WorkspaceActivity::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(activities)
    }

    async fn cleanup_old_activities(&mut self, days_to_keep: i64) -> PgResult<usize> {
        use schema::workspace_activities::dsl::*;

        let cutoff_date =
            jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().days(days_to_keep));

        let deleted_count = diesel::delete(workspace_activities)
            .filter(created_at.lt(cutoff_date))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(deleted_count)
    }
}
