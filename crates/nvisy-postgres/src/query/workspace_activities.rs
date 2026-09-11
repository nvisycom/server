//! Workspace activity repository for managing workspace activity log operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspaceActivity, WorkspaceActivity};
use crate::types::{
    AccountRefRow, ActivityType, CursorPage, CursorPagination, WithAccountRef, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating a workspace's activity log: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityCursor {
    /// When the activity was recorded.
    pub created_at: Timestamp,
    /// Activity id (tiebreaker).
    pub id: uuid::Uuid,
}

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

/// Repository for workspace activity log database operations.
///
/// Handles activity logging, querying, and audit trail management.
pub trait WorkspaceActivityRepository {
    /// Logs a new activity in the workspace activity log.
    fn log_activity(
        &mut self,
        activity: NewWorkspaceActivity,
    ) -> impl Future<Output = Result<WorkspaceActivity>> + Send;

    /// Lists a workspace's activities with cursor pagination, newest first, each
    /// paired with the handle and avatar of the account that performed it. The
    /// `filter` narrows by type, actor, and/or time window (an empty filter lists
    /// everything).
    fn cursor_list_workspace_activity(
        &mut self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        pagination: CursorPagination<ActivityCursor>,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceActivity>>>> + Send;

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
    ) -> impl Future<Output = Result<Vec<WithAccountRef<WorkspaceActivity>>>> + Send;
}

impl WorkspaceActivityRepository for PgConnection {
    async fn log_activity(&mut self, activity: NewWorkspaceActivity) -> Result<WorkspaceActivity> {
        use schema::workspace_activities;

        let activity = diesel::insert_into(workspace_activities::table)
            .values(&activity)
            .returning(WorkspaceActivity::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(activity)
    }

    async fn cursor_list_workspace_activity(
        &mut self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        pagination: CursorPagination<ActivityCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceActivity>>> {
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
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let query = apply_activity_filter(
            workspace_activities::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .into_boxed(),
            &filter,
        )
        .inner_join(accounts::table);

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspaceActivity, AccountRefRow)> =
            keyset!(query, dsl::created_at, dsl::id, pagination.direction, after)
                .select((
                    WorkspaceActivity::as_select(),
                    (
                        accounts::username,
                        accounts::display_name,
                        accounts::avatar_url,
                    ),
                ))
                .limit(pagination.fetch_limit())
                .load(self)
                .await
                .map_err(Error::from)?;

        let items: Vec<WithAccountRef<WorkspaceActivity>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            ActivityCursor {
                created_at: wc.item.created_at.into(),
                id: wc.item.id,
            }
        }))
    }

    async fn list_workspace_activity_for_export(
        &mut self,
        workspace_id: Uuid,
        filter: ActivityFilter,
        limit: i64,
    ) -> Result<Vec<WithAccountRef<WorkspaceActivity>>> {
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
            .map_err(Error::from)?;

        Ok(rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect())
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

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};

    use super::*;
    use crate::PgConn;
    use crate::model::{NewAccount, NewWorkspaceActivity};
    use crate::query::{AccountRepository, WorkspaceActivityRepository};
    use crate::test_util::{TestDatabase, backdate};

    /// Logs an activity of `activity_type` for `workspace_id` by `account_id`,
    /// overriding the type on the default test payload so the type filter has a
    /// distinguishable value. `age` backdates its `created_at` so ordering is
    /// deterministic.
    async fn log(
        conn: &mut PgConn,
        workspace_id: Uuid,
        account_id: Uuid,
        activity_type: ActivityType,
        age: Option<Span>,
    ) -> anyhow::Result<WorkspaceActivity> {
        let mut activity = NewWorkspaceActivity::test(workspace_id, account_id);
        activity.activity_type = activity_type;
        let logged = conn.log_activity(activity).await?;
        if let Some(span) = age {
            backdate::activity_created_at(conn, logged.id, Timestamp::now() - span).await?;
        }
        Ok(logged)
    }

    #[tokio::test]
    async fn feed_lists_newest_first_and_export_lists_oldest_first() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // `first` is an hour old so the newest-first / oldest-first orders are
        // deterministic against `second`.
        let first = log(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            ActivityType::WorkspaceCreated,
            Some(Span::new().hours(1)),
        )
        .await?;
        let second = log(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            ActivityType::WorkspaceUpdated,
            None,
        )
        .await?;

        // The paginated feed is newest first.
        let feed = conn
            .cursor_list_workspace_activity(
                seeded.workspace_id,
                ActivityFilter::default(),
                CursorPagination::new(50),
            )
            .await?;
        assert_eq!(
            feed.items.iter().map(|a| a.item.id).collect::<Vec<_>>(),
            vec![second.id, first.id]
        );

        // The export is oldest first.
        let export = conn
            .list_workspace_activity_for_export(seeded.workspace_id, ActivityFilter::default(), 50)
            .await?;
        assert_eq!(
            export.iter().map(|a| a.item.id).collect::<Vec<_>>(),
            vec![first.id, second.id]
        );
        Ok(())
    }

    #[tokio::test]
    async fn filter_by_type_and_actor() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A second actor in the same workspace.
        let other_id = conn.create_account(NewAccount::test()).await?.id;

        let created = log(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            ActivityType::WorkspaceCreated,
            None,
        )
        .await?;
        let _updated = log(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            ActivityType::WorkspaceUpdated,
            None,
        )
        .await?;
        let by_other = log(
            &mut conn,
            seeded.workspace_id,
            other_id,
            ActivityType::WorkspaceCreated,
            None,
        )
        .await?;

        // Type filter keeps only WorkspaceCreated (from either actor).
        let created_only = conn
            .cursor_list_workspace_activity(
                seeded.workspace_id,
                ActivityFilter {
                    types: vec![ActivityType::WorkspaceCreated],
                    ..Default::default()
                },
                CursorPagination::new(50),
            )
            .await?;
        let mut ids: Vec<_> = created_only.items.iter().map(|a| a.item.id).collect();
        ids.sort();
        let mut expected = vec![created.id, by_other.id];
        expected.sort();
        assert_eq!(ids, expected);

        // Actor filter keeps only the other actor's activity.
        let others = conn
            .cursor_list_workspace_activity(
                seeded.workspace_id,
                ActivityFilter {
                    actor: Some(other_id),
                    ..Default::default()
                },
                CursorPagination::new(50),
            )
            .await?;
        assert_eq!(
            others.items.iter().map(|a| a.item.id).collect::<Vec<_>>(),
            vec![by_other.id]
        );
        Ok(())
    }
}
