//! Workspace connections repository for managing encrypted provider connections.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspaceConnection, UpdateWorkspaceConnection, WorkspaceConnection};
use crate::types::{AccountRefRow, CursorPage, CursorPagination, WithAccountRef, keyset};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating a workspace's connections: newest first by `created_at`,
/// `id` as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionCursor {
    /// When the connection was created.
    pub created_at: Timestamp,
    /// Connection id (tiebreaker).
    pub id: uuid::Uuid,
}

/// A sync-scheduled connection paired with its cron expression, as returned by
/// [`WorkspaceConnectionRepository::list_scheduled_connections`]. The cron is
/// non-optional: the query only lists connections whose schedule has one. The
/// direction is not carried — the consumer re-reads it from the live schedule
/// when it opens the run, so a mid-flight direction change is honored.
#[derive(Debug, Clone, Queryable)]
pub struct ScheduledConnection {
    /// The connection due for scheduling.
    pub connection: WorkspaceConnection,
    /// The connection's cron expression.
    pub schedule_cron: String,
}

/// Repository for workspace connection database operations.
///
/// Handles connection lifecycle management including creation, updates,
/// and workspace-scoped queries.
pub trait WorkspaceConnectionRepository {
    /// Creates a new workspace connection record.
    fn create_workspace_connection(
        &mut self,
        new_connection: NewWorkspaceConnection,
    ) -> impl Future<Output = Result<WorkspaceConnection>> + Send;

    /// Finds a connection by id and takes a row lock (`SELECT ... FOR UPDATE`)
    /// for the current transaction.
    ///
    /// Use this on the read of any read-modify-write of `encrypted_data` (token
    /// refresh, config replace) so concurrent writers serialize and neither
    /// overwrites the other from a stale snapshot. Must run inside a transaction.
    fn find_workspace_connection_by_id_for_update(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceConnection>>> + Send;

    /// Finds a connection by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_connection_in_workspace(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceConnection>>> + Send;

    /// Finds a connection by id within a specific workspace, with the handle and
    /// avatar of the account that created it.
    ///
    /// Excludes soft-deleted connections.
    fn find_connection_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Option<WithAccountRef<WorkspaceConnection>>>> + Send;

    /// Lists all active connections that have a cron sync schedule, in either
    /// direction, across every workspace, each paired with its cron. Used by the
    /// scheduled-sync worker; returning the cron avoids re-reading each schedule
    /// row.
    fn list_scheduled_connections(
        &mut self,
    ) -> impl Future<Output = Result<Vec<ScheduledConnection>>> + Send;

    /// Lists all connections in a workspace with cursor pagination, each paired
    /// with the handle and avatar of the account that created it.
    ///
    /// An empty `providers` slice means no provider filter; otherwise a
    /// connection matches if its provider is any of the given ones.
    fn cursor_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<ConnectionCursor>,
        providers: &[String],
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceConnection>>>> + Send;

    /// Updates a connection with new data.
    fn update_workspace_connection(
        &mut self,
        connection_id: Uuid,
        updates: UpdateWorkspaceConnection,
    ) -> impl Future<Output = Result<WorkspaceConnection>> + Send;

    /// Soft deletes a connection by setting the deletion timestamp.
    fn delete_workspace_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl WorkspaceConnectionRepository for PgConnection {
    async fn create_workspace_connection(
        &mut self,
        new_connection: NewWorkspaceConnection,
    ) -> Result<WorkspaceConnection> {
        use schema::workspace_connections;

        let connection = diesel::insert_into(workspace_connections::table)
            .values(&new_connection)
            .returning(WorkspaceConnection::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(connection)
    }

    async fn find_workspace_connection_by_id_for_update(
        &mut self,
        connection_id: Uuid,
    ) -> Result<Option<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connection = workspace_connections::table
            .filter(dsl::id.eq(connection_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceConnection::as_select())
            .for_update()
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(connection)
    }

    async fn find_connection_in_workspace(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Option<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connection = workspace_connections::table
            .filter(dsl::id.eq(connection_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceConnection::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(connection)
    }

    async fn find_connection_in_workspace_with_creator(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<Option<WithAccountRef<WorkspaceConnection>>> {
        use schema::workspace_connections::dsl;
        use schema::{accounts, workspace_connections};

        let row = workspace_connections::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::id.eq(connection_id))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceConnection::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(WorkspaceConnection, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn list_scheduled_connections(&mut self) -> Result<Vec<ScheduledConnection>> {
        use schema::workspace_connection_schedule as sched;
        use schema::workspace_connections::{self, dsl};

        // Scheduled-sync config lives in the schedule satellite; join it to find
        // every active connection with a cron schedule, in either direction. The
        // `schedule_cron IS NOT NULL` filter makes the column non-null for this
        // query, so the worker gets the cron without re-reading the schedule row.
        // Which connections are given a schedule is decided at the handler layer.
        let connections = workspace_connections::table
            .inner_join(sched::table.on(sched::connection_id.eq(dsl::id)))
            .filter(sched::schedule_cron.is_not_null())
            .filter(dsl::is_active.eq(true))
            .filter(dsl::deleted_at.is_null())
            .select((
                WorkspaceConnection::as_select(),
                sched::schedule_cron.assume_not_null(),
            ))
            .load::<ScheduledConnection>(self)
            .await
            .map_err(Error::from)?;

        Ok(connections)
    }

    async fn cursor_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<ConnectionCursor>,
        providers: &[String],
    ) -> Result<CursorPage<WithAccountRef<WorkspaceConnection>>> {
        use schema::workspace_connections::dsl;
        use schema::{accounts, workspace_connections};

        // The scoped builder (filters shared by the count and the page).
        let scoped = || {
            let mut query = workspace_connections::table
                .inner_join(accounts::table)
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .into_boxed();
            if !providers.is_empty() {
                query = query.filter(dsl::provider.eq_any(providers.to_vec()));
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.id));
        let rows: Vec<(WorkspaceConnection, AccountRefRow)> = keyset!(
            scoped(),
            dsl::created_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select((
            WorkspaceConnection::as_select(),
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

        let items: Vec<WithAccountRef<WorkspaceConnection>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            ConnectionCursor {
                created_at: wc.item.created_at.into(),
                id: wc.item.id,
            }
        }))
    }

    async fn update_workspace_connection(
        &mut self,
        connection_id: Uuid,
        updates: UpdateWorkspaceConnection,
    ) -> Result<WorkspaceConnection> {
        use schema::workspace_connections::{self, dsl};

        // Scope to a live row: a concurrent delete may have committed since the
        // caller's lookup, and updating the tombstoned row would revive it in
        // effect and emit a spurious event.
        let connection = diesel::update(
            workspace_connections::table
                .filter(dsl::id.eq(connection_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(&updates)
        .returning(WorkspaceConnection::as_returning())
        .get_result(self)
        .await
        .map_err(Error::from)?;

        Ok(connection)
    }

    async fn delete_workspace_connection(&mut self, connection_id: Uuid) -> Result<()> {
        use diesel::dsl::now;
        use schema::workspace_connections::{self, dsl};

        // Scope to a live row so a concurrent delete is not overwritten with a
        // fresh `deleted_at`.
        diesel::update(
            workspace_connections::table
                .filter(dsl::id.eq(connection_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::AsyncConnection;
    use crate::model::{
        NewWorkspaceConnection, NewWorkspaceConnectionSchedule, UpdateWorkspaceConnection,
    };
    use crate::query::{WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository};
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn create_and_scoped_lookups_round_trip() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let connection = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // Found within its own workspace.
        assert!(
            conn.find_connection_in_workspace(seeded.workspace_id, connection.id)
                .await?
                .is_some()
        );
        // Not found in another workspace (workspace-scoped access control).
        assert!(
            conn.find_connection_in_workspace(Uuid::now_v7(), connection.id)
                .await?
                .is_none()
        );

        // The creator join returns the connection with its creator's handle.
        let with_creator = conn
            .find_connection_in_workspace_with_creator(seeded.workspace_id, connection.id)
            .await?
            .expect("connection should be present");
        assert_eq!(with_creator.item.id, connection.id);

        // The locking read inside a transaction returns the row.
        let locked = conn
            .transaction(async |conn| {
                conn.find_workspace_connection_by_id_for_update(connection.id)
                    .await
            })
            .await?;
        assert_eq!(locked.map(|c| c.id), Some(connection.id));
        Ok(())
    }

    #[tokio::test]
    async fn soft_delete_hides_the_row_from_reads_and_updates() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let connection = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // An update applies to a live row.
        let renamed = conn
            .update_workspace_connection(
                connection.id,
                UpdateWorkspaceConnection {
                    display_name: Some("Renamed".to_owned()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(renamed.display_name, "Renamed");

        conn.delete_workspace_connection(connection.id).await?;

        // Every scoped read now excludes it.
        assert!(
            conn.find_connection_in_workspace(seeded.workspace_id, connection.id)
                .await?
                .is_none()
        );
        assert!(
            conn.find_connection_in_workspace_with_creator(seeded.workspace_id, connection.id)
                .await?
                .is_none()
        );
        let locked = conn
            .transaction(async |conn| {
                conn.find_workspace_connection_by_id_for_update(connection.id)
                    .await
            })
            .await?;
        assert!(locked.is_none());

        // Updating the tombstoned row affects nothing (no live row to return).
        let update = conn
            .update_workspace_connection(
                connection.id,
                UpdateWorkspaceConnection {
                    display_name: Some("Revived".to_owned()),
                    ..Default::default()
                },
            )
            .await;
        assert!(
            update.is_err(),
            "update of a deleted row should not succeed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_filters_by_provider_and_excludes_deleted() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // An s3 connection, an azure connection, and a deleted s3 connection.
        let s3 = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let mut azure = NewWorkspaceConnection::test(seeded.workspace_id, seeded.account_id);
        azure.provider = "azure".to_owned();
        let azure = conn.create_workspace_connection(azure).await?;
        let deleted = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        conn.delete_workspace_connection(deleted.id).await?;

        // No provider filter: both live connections, deleted excluded.
        let all = conn
            .cursor_list_workspace_connections(seeded.workspace_id, CursorPagination::new(50), &[])
            .await?;
        let ids: Vec<_> = all.items.iter().map(|c| c.item.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&s3.id) && ids.contains(&azure.id));
        assert!(!ids.contains(&deleted.id));

        // Filtered to azure only.
        let azure_only = conn
            .cursor_list_workspace_connections(
                seeded.workspace_id,
                CursorPagination::new(50),
                &["azure".to_owned()],
            )
            .await?;
        assert_eq!(
            azure_only
                .items
                .iter()
                .map(|c| c.item.id)
                .collect::<Vec<_>>(),
            vec![azure.id]
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_scheduled_connections_requires_active_cron_and_not_deleted() -> anyhow::Result<()>
    {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A scheduled, active connection: it should be listed with its cron.
        let scheduled = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let mut schedule = NewWorkspaceConnectionSchedule::test(scheduled.id);
        schedule.schedule_cron = Some("0 * * * *".to_owned());
        let _ = conn.create_connection_schedule(schedule).await?;

        // A connection whose schedule has no cron (manual-only): excluded.
        let manual = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let _ = conn
            .create_connection_schedule(NewWorkspaceConnectionSchedule::test(manual.id))
            .await?;

        // An inactive connection with a cron schedule: excluded.
        let mut inactive = NewWorkspaceConnection::test(seeded.workspace_id, seeded.account_id);
        inactive.is_active = Some(false);
        let inactive = conn.create_workspace_connection(inactive).await?;
        let mut inactive_schedule = NewWorkspaceConnectionSchedule::test(inactive.id);
        inactive_schedule.schedule_cron = Some("0 * * * *".to_owned());
        let _ = conn.create_connection_schedule(inactive_schedule).await?;

        let listed = conn.list_scheduled_connections().await?;
        let ids: Vec<_> = listed.iter().map(|s| s.connection.id).collect();
        assert_eq!(ids, vec![scheduled.id]);
        assert_eq!(listed[0].schedule_cron, "0 * * * *");
        Ok(())
    }
}
