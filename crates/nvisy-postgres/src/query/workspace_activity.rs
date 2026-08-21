//! Workspace activity repository for managing workspace activity log operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use ipnet::IpNet;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use crate::model::{NewWorkspaceActivity, WorkspaceActivity};
use crate::types::{
    AccountRefRow, ActivityPayload, ActivityType, CursorPage, CursorPagination, Json,
    OffsetPagination, WithAccountRef,
};
use crate::{PgConnection, PgError, PgResult, schema};

/// Predicates that narrow an activity listing, all optional (an empty filter
/// matches every activity in the workspace). Shared by the paginated feed and the
/// export so both apply the same constraints.
#[derive(Debug, Clone, Default)]
pub struct ActivityFilter {
    /// Keep only these activity types. Empty means no type constraint.
    pub types: Vec<ActivityType>,
    /// Keep only activities performed by this account. `None` means any actor.
    pub actor: Option<Uuid>,
    /// Keep only activities at or after this instant (inclusive). `None` means
    /// no lower bound.
    pub from: Option<Timestamp>,
    /// Keep only activities strictly before this instant (exclusive). `None`
    /// means no upper bound.
    pub to: Option<Timestamp>,
}

/// Parameters for logging entity-specific activities.
#[derive(Debug, Clone)]
pub struct LogEntityActivityParams {
    /// The account that performed the activity.
    pub account_id: Uuid,
    /// The type of activity being logged.
    pub activity_type: ActivityType,
    /// The self-describing tagged payload (its `activityType` + params).
    pub params: Json<ActivityPayload>,
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

    /// Lists a workspace's activities with cursor pagination, newest first, each
    /// paired with the handle and avatar of the account that performed it. The
    /// `filter` narrows by type, actor, and/or time window (an empty filter lists
    /// everything).
    fn cursor_list_workspace_activity(
        &mut self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WithAccountRef<WorkspaceActivity>>>> + Send;

    /// Lists a workspace's filtered activities oldest first (the natural order for
    /// an export), each paired with the performing account's handle and avatar. At
    /// most `limit` rows are returned; the caller sets `limit` one above its cap so
    /// a full result signals truncation. The `filter` applies the same type/actor/
    /// window constraints as the feed.
    fn list_workspace_activity_for_export(
        &mut self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<WithAccountRef<WorkspaceActivity>>>> + Send;

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
    ) -> impl Future<Output = PgResult<Vec<(Uuid, i64)>>> + Send;

    /// Gets a breakdown of activities by type for analytical reporting.
    fn get_activity_type_breakdown(
        &mut self,
        workspace_id: Uuid,
        hours: Option<i64>,
    ) -> impl Future<Output = PgResult<Vec<(ActivityType, i64)>>> + Send;

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
        filter: ActivityFilter,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WithAccountRef<WorkspaceActivity>>> {
        use diesel::dsl::count_star;
        use schema::workspace_activities::dsl;
        use schema::{accounts, workspace_activities};

        // Count over the same filter, only when requested.
        let total = if pagination.include_count {
            let count_query = workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .into_boxed();
            Some(
                apply_activity_filter(count_query, &filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let mut query = apply_activity_filter(
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .into_boxed(),
            &filter,
        );

        if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            query = query.filter(
                dsl::created_at
                    .lt(cursor_ts)
                    .or(dsl::created_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
            );
        }

        let rows: Vec<(WorkspaceActivity, AccountRefRow)> = query
            .inner_join(accounts::table)
            .select((
                WorkspaceActivity::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .order((dsl::created_at.desc(), dsl::id.desc()))
            .limit(pagination.fetch_limit())
            .load(self)
            .await
            .map_err(PgError::from)?;

        let items: Vec<WithAccountRef<WorkspaceActivity>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            (wc.item.created_at.into(), wc.item.id)
        }))
    }

    async fn list_workspace_activity_for_export(
        &mut self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        limit: i64,
    ) -> PgResult<Vec<WithAccountRef<WorkspaceActivity>>> {
        use schema::workspace_activities::dsl;
        use schema::{accounts, workspace_activities};

        let query = apply_activity_filter(
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .into_boxed(),
            &filter,
        );

        let rows: Vec<(WorkspaceActivity, AccountRefRow)> = query
            .inner_join(accounts::table)
            .select((
                WorkspaceActivity::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .order((dsl::created_at.asc(), dsl::id.asc()))
            .limit(limit)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect())
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
            params: params.params,
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
            params: params.params,
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
            params: params.params,
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
    ) -> PgResult<Vec<(Uuid, i64)>> {
        use schema::workspace_activities::{self, dsl};

        let results = if let Some(time_window) = hours {
            let cutoff_time =
                jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(time_window));
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::created_at.gt(cutoff_time))
                .group_by(dsl::account_id)
                .select((dsl::account_id, diesel::dsl::count(dsl::id)))
                .order(diesel::dsl::count(dsl::id).desc())
                .limit(limit)
                .load::<(Uuid, i64)>(self)
                .await
                .map_err(PgError::from)?
        } else {
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .group_by(dsl::account_id)
                .select((dsl::account_id, diesel::dsl::count(dsl::id)))
                .order(diesel::dsl::count(dsl::id).desc())
                .limit(limit)
                .load::<(Uuid, i64)>(self)
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

/// Applies an [`ActivityFilter`]'s predicates (type, actor, time window) to a
/// boxed activity query. Kept separate so the paginated feed and the export apply
/// identical constraints; an empty filter is a no-op.
fn apply_activity_filter<'a>(
    mut query: schema::workspace_activities::BoxedQuery<'a, diesel::pg::Pg>,
    filter: &ActivityFilter,
) -> schema::workspace_activities::BoxedQuery<'a, diesel::pg::Pg> {
    use schema::workspace_activities::dsl;

    if !filter.types.is_empty() {
        query = query.filter(dsl::activity_type.eq_any(filter.types.clone()));
    }
    if let Some(actor) = filter.actor {
        query = query.filter(dsl::account_id.eq(actor));
    }
    if let Some(from) = filter.from {
        query = query.filter(dsl::created_at.ge(jiff_diesel::Timestamp::from(from)));
    }
    if let Some(to) = filter.to {
        query = query.filter(dsl::created_at.lt(jiff_diesel::Timestamp::from(to)));
    }
    query
}
