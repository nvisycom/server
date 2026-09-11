//! Workspace connection syncs repository for managing sync execution instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspaceConnectionSync, WorkspaceConnectionSync};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, SyncStatus, WithAccountRef, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating connection sync runs: newest first by `started_at`, `id`
/// as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSyncCursor {
    /// When the sync run started.
    pub started_at: Timestamp,
    /// Sync run id (tiebreaker).
    pub id: uuid::Uuid,
}

/// Repository for workspace connection sync database operations.
///
/// Handles sync lifecycle management including creation, status updates,
/// completion tracking, and queries.
pub trait WorkspaceConnectionSyncRepository {
    /// Creates a new workspace connection sync record.
    fn create_workspace_connection_sync(
        &mut self,
        new_sync: NewWorkspaceConnectionSync,
    ) -> impl Future<Output = Result<WorkspaceConnectionSync>> + Send;

    /// Finds a workspace connection sync by its unique identifier.
    fn find_workspace_connection_sync_by_id(
        &mut self,
        sync_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceConnectionSync>>> + Send;

    /// Finds a sync by ID, scoped to a workspace via its owning connection.
    ///
    /// Runs carry no workspace column, so this joins through the connection and
    /// filters on its workspace. A sync whose connection is in another workspace
    /// is not found.
    fn find_connection_sync_in_workspace(
        &mut self,
        workspace_id: Uuid,
        sync_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceConnectionSync>>> + Send;

    /// Returns the most recent successful sync completion time for each of the
    /// given connections.
    ///
    /// A connection's "last synced" instant is the `completed_at` of its latest
    /// sync with status `Completed`; connections that have never synced
    /// successfully are absent from the result. This is a single grouped query
    /// so a page of connections costs one round-trip, not one per connection.
    fn last_successful_sync_at(
        &mut self,
        connection_ids: &[Uuid],
    ) -> impl Future<Output = Result<Vec<(Uuid, jiff_diesel::Timestamp)>>> + Send;

    /// Lists runs for a specific connection with cursor pagination, each paired
    /// with the account that triggered it.
    fn cursor_list_workspace_connection_syncs(
        &mut self,
        connection_id: Uuid,
        pagination: CursorPagination<ConnectionSyncCursor>,
        status_filter: Option<SyncStatus>,
    ) -> impl Future<Output = Result<CursorPage<WithAccountRef<WorkspaceConnectionSync>>>> + Send;

    /// Lists all runs across a workspace's connections with cursor pagination.
    ///
    /// Runs carry no workspace reference of their own, so this joins through the
    /// owning connection and filters on its workspace. An optional status filter
    /// and a set of providers narrow the result; an empty `providers` slice means
    /// no provider filter. Use [`cursor_list_workspace_connection_syncs`] for a
    /// single connection.
    ///
    /// [`cursor_list_workspace_connection_syncs`]: Self::cursor_list_workspace_connection_syncs
    fn cursor_list_workspace_connection_syncs_all(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<ConnectionSyncCursor>,
        status_filter: Option<SyncStatus>,
        providers: &[String],
    ) -> impl Future<Output = Result<CursorPage<(WithAccountRef<WorkspaceConnectionSync>, Uuid)>>> + Send;

    /// Gets the most recent sync for a connection (its current sync state).
    fn find_latest_workspace_connection_sync(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceConnectionSync>>> + Send;

    /// Gets the most recent sync for each of `connection_ids` in one query.
    /// Connections with no sync yet are simply absent from the result. Used by
    /// the scheduled-sync worker to snapshot every candidate's state at once.
    fn find_latest_workspace_connection_syncs(
        &mut self,
        connection_ids: &[Uuid],
    ) -> impl Future<Output = Result<Vec<WorkspaceConnectionSync>>> + Send;

    /// Marks a sync as completed successfully with its final record count, only if
    /// it is still active.
    ///
    /// Returns the updated sync, or `None` if the sync was already in a terminal
    /// state (e.g. cancelled or reaped) and was therefore left unchanged. The
    /// count is written under the same status guard so a terminal sync's fields
    /// are never mutated after the fact.
    fn complete_workspace_connection_sync(
        &mut self,
        sync_id: Uuid,
        records_synced: i64,
    ) -> impl Future<Output = Result<Option<WorkspaceConnectionSync>>> + Send;

    /// Marks a sync as failed, recording the error detail, only if it is still
    /// active. Returns `None` if the sync was already terminal.
    fn fail_workspace_connection_sync(
        &mut self,
        sync_id: Uuid,
        error_message: &str,
    ) -> impl Future<Output = Result<Option<WorkspaceConnectionSync>>> + Send;

    /// Marks a sync as cancelled, only if it is still active. Returns `None` if
    /// the sync was already terminal and was therefore left unchanged.
    fn cancel_workspace_connection_sync(
        &mut self,
        sync_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceConnectionSync>>> + Send;

    /// Fails all `Running` runs that started before `cutoff`, returning the
    /// number reaped. Recovers runs orphaned by a crash mid-sync.
    fn fail_stale_running_syncs(
        &mut self,
        cutoff: jiff_diesel::Timestamp,
    ) -> impl Future<Output = Result<usize>> + Send;
}

impl WorkspaceConnectionSyncRepository for PgConnection {
    async fn create_workspace_connection_sync(
        &mut self,
        new_sync: NewWorkspaceConnectionSync,
    ) -> Result<WorkspaceConnectionSync> {
        use schema::workspace_connection_syncs;

        let sync = diesel::insert_into(workspace_connection_syncs::table)
            .values(&new_sync)
            .returning(WorkspaceConnectionSync::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(sync)
    }

    async fn find_workspace_connection_sync_by_id(
        &mut self,
        sync_id: Uuid,
    ) -> Result<Option<WorkspaceConnectionSync>> {
        use schema::workspace_connection_syncs::{self, dsl};

        let sync = workspace_connection_syncs::table
            .filter(dsl::id.eq(sync_id))
            .select(WorkspaceConnectionSync::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(sync)
    }

    async fn find_connection_sync_in_workspace(
        &mut self,
        workspace_id: Uuid,
        sync_id: Uuid,
    ) -> Result<Option<WorkspaceConnectionSync>> {
        use schema::workspace_connection_syncs::dsl as runs;
        use schema::workspace_connections::dsl as connections;

        let sync = runs::workspace_connection_syncs
            .inner_join(
                connections::workspace_connections.on(connections::id.eq(runs::connection_id)),
            )
            .filter(runs::id.eq(sync_id))
            .filter(connections::workspace_id.eq(workspace_id))
            .select(WorkspaceConnectionSync::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(sync)
    }

    async fn last_successful_sync_at(
        &mut self,
        connection_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, jiff_diesel::Timestamp)>> {
        use diesel::dsl::max;
        use schema::workspace_connection_syncs::{self, dsl};

        if connection_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Only successful runs count toward "last synced"; a failed or cancelled
        // sync does not move the timestamp. completed_at is non-null for any sync
        // in a terminal state, so the grouped MAX is present for every group.
        workspace_connection_syncs::table
            .filter(dsl::connection_id.eq_any(connection_ids))
            .filter(dsl::status.eq(SyncStatus::Completed))
            .group_by(dsl::connection_id)
            .select((dsl::connection_id, max(dsl::completed_at).assume_not_null()))
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn cursor_list_workspace_connection_syncs(
        &mut self,
        connection_id: Uuid,
        pagination: CursorPagination<ConnectionSyncCursor>,
        status_filter: Option<SyncStatus>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceConnectionSync>>> {
        use schema::workspace_connection_syncs::dsl;
        use schema::{accounts, workspace_connection_syncs};

        // The scoped builder (filters shared by the count and the page).
        let scoped = || {
            let mut query = workspace_connection_syncs::table
                .inner_join(accounts::table)
                .filter(dsl::connection_id.eq(connection_id))
                .into_boxed();
            if let Some(status) = status_filter {
                query = query.filter(dsl::status.eq(status));
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
            .map(|k| (jiff_diesel::Timestamp::from(k.started_at), k.id));
        let rows: Vec<(WorkspaceConnectionSync, AccountRefRow)> = keyset!(
            scoped(),
            dsl::started_at,
            dsl::id,
            pagination.direction,
            after
        )
        .select((
            WorkspaceConnectionSync::as_select(),
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

        let items: Vec<WithAccountRef<WorkspaceConnectionSync>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            ConnectionSyncCursor {
                started_at: wc.item.started_at.into(),
                id: wc.item.id,
            }
        }))
    }

    async fn cursor_list_workspace_connection_syncs_all(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<ConnectionSyncCursor>,
        status_filter: Option<SyncStatus>,
        providers: &[String],
    ) -> Result<CursorPage<(WithAccountRef<WorkspaceConnectionSync>, Uuid)>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_connection_syncs::dsl as runs;
        use schema::workspace_connections::dsl as connections;

        // Runs have no workspace column; scope them through the owning
        // connection. The owning connection's id and the triggering account are
        // selected alongside each sync so the cross-connection response can name
        // its connection and trigger (the sync is addressed by its own id).
        let scoped = || {
            let mut query = runs::workspace_connection_syncs
                .inner_join(
                    connections::workspace_connections.on(connections::id.eq(runs::connection_id)),
                )
                .inner_join(accounts::accounts)
                .filter(connections::workspace_id.eq(workspace_id))
                .into_boxed();
            if let Some(status) = status_filter {
                query = query.filter(runs::status.eq(status));
            }
            if !providers.is_empty() {
                query = query.filter(connections::provider.eq_any(providers.to_vec()));
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

        let selection = (
            WorkspaceConnectionSync::as_select(),
            connections::id,
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        );

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.started_at), k.id));
        let rows: Vec<(WorkspaceConnectionSync, Uuid, AccountRefRow)> = keyset!(
            scoped(),
            runs::started_at,
            runs::id,
            pagination.direction,
            after
        )
        .select(selection)
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        let items: Vec<(WithAccountRef<WorkspaceConnectionSync>, Uuid)> = rows
            .into_iter()
            .map(|(item, connection_id, account)| (WithAccountRef { item, account }, connection_id))
            .collect();

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |(wc, _): &(WithAccountRef<WorkspaceConnectionSync>, Uuid)| ConnectionSyncCursor {
                started_at: wc.item.started_at.into(),
                id: wc.item.id,
            },
        ))
    }

    async fn find_latest_workspace_connection_sync(
        &mut self,
        connection_id: Uuid,
    ) -> Result<Option<WorkspaceConnectionSync>> {
        use schema::workspace_connection_syncs::{self, dsl};

        let sync = workspace_connection_syncs::table
            .filter(dsl::connection_id.eq(connection_id))
            // `id` breaks a `started_at` tie so the newest row is deterministic,
            // matching the batched `find_latest_workspace_connection_syncs`.
            .order((dsl::started_at.desc(), dsl::id.desc()))
            .select(WorkspaceConnectionSync::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(sync)
    }

    async fn find_latest_workspace_connection_syncs(
        &mut self,
        connection_ids: &[Uuid],
    ) -> Result<Vec<WorkspaceConnectionSync>> {
        use schema::workspace_connection_syncs::{self, dsl};

        // One latest row per connection: DISTINCT ON keeps the first row for each
        // connection_id under the matching ORDER BY (newest started_at first). The
        // id tie-breaker makes the pick deterministic when two runs share a
        // started_at, so the scheduler reads a stable busy state / last attempt.
        let syncs = workspace_connection_syncs::table
            .filter(dsl::connection_id.eq_any(connection_ids))
            .distinct_on(dsl::connection_id)
            .order((dsl::connection_id, dsl::started_at.desc(), dsl::id.desc()))
            .select(WorkspaceConnectionSync::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(syncs)
    }

    async fn complete_workspace_connection_sync(
        &mut self,
        sync_id: Uuid,
        records_synced: i64,
    ) -> Result<Option<WorkspaceConnectionSync>> {
        use diesel::dsl::now;
        use schema::workspace_connection_syncs::{self, dsl};

        // Only transition from an active state, so a sync already cancelled/reaped
        // is not resurrected as completed and its record count is not rewritten.
        let sync = diesel::update(
            workspace_connection_syncs::table
                .filter(dsl::id.eq(sync_id))
                .filter(dsl::status.eq_any([SyncStatus::Pending, SyncStatus::Running])),
        )
        .set((
            dsl::status.eq(SyncStatus::Completed),
            dsl::records_synced.eq(records_synced),
            dsl::completed_at.eq(now),
        ))
        .returning(WorkspaceConnectionSync::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(Error::from)?;

        Ok(sync)
    }

    async fn fail_workspace_connection_sync(
        &mut self,
        sync_id: Uuid,
        error_message: &str,
    ) -> Result<Option<WorkspaceConnectionSync>> {
        use diesel::dsl::now;
        use schema::workspace_connection_syncs::{self, dsl};

        // Only transition from an active state, so a terminal sync is not
        // overwritten.
        let sync = diesel::update(
            workspace_connection_syncs::table
                .filter(dsl::id.eq(sync_id))
                .filter(dsl::status.eq_any([SyncStatus::Pending, SyncStatus::Running])),
        )
        .set((
            dsl::status.eq(SyncStatus::Failed),
            dsl::error_message.eq(error_message),
            dsl::completed_at.eq(now),
        ))
        .returning(WorkspaceConnectionSync::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(Error::from)?;

        Ok(sync)
    }

    async fn cancel_workspace_connection_sync(
        &mut self,
        sync_id: Uuid,
    ) -> Result<Option<WorkspaceConnectionSync>> {
        use diesel::dsl::now;
        use schema::workspace_connection_syncs::{self, dsl};

        // Only transition from an active state, so a sync that already completed,
        // failed, or was reaped is not overwritten as cancelled.
        let sync = diesel::update(
            workspace_connection_syncs::table
                .filter(dsl::id.eq(sync_id))
                .filter(dsl::status.eq_any([SyncStatus::Pending, SyncStatus::Running])),
        )
        .set((
            dsl::status.eq(SyncStatus::Cancelled),
            dsl::completed_at.eq(now),
        ))
        .returning(WorkspaceConnectionSync::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(Error::from)?;

        Ok(sync)
    }

    async fn fail_stale_running_syncs(&mut self, cutoff: jiff_diesel::Timestamp) -> Result<usize> {
        use diesel::dsl::now;
        use schema::workspace_connection_syncs::{self, dsl};

        let reaped = diesel::update(
            workspace_connection_syncs::table
                .filter(dsl::status.eq(SyncStatus::Running))
                .filter(dsl::started_at.lt(cutoff)),
        )
        .set((
            dsl::status.eq(SyncStatus::Failed),
            dsl::error_message.eq("Sync did not complete (reaped as stale)"),
            dsl::completed_at.eq(now),
        ))
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(reaped)
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use uuid::Uuid;

    use super::*;
    use crate::PgConn;
    use crate::model::{NewWorkspaceConnection, NewWorkspaceConnectionSync};
    use crate::query::{WorkspaceConnectionRepository, WorkspaceConnectionSyncRepository};
    use crate::test_util::{TestDatabase, backdate};

    /// Seeds a connection in a fresh workspace, returning `(account_id,
    /// workspace_id, connection_id)` — the FK parents a sync requires.
    async fn seed_connection(db: &TestDatabase) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let connection = conn
            .create_workspace_connection(NewWorkspaceConnection::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        Ok((seeded.account_id, seeded.workspace_id, connection.id))
    }

    /// Creates a sync whose `started_at` is `ago` in the past, so time-based
    /// queries treat it as old.
    async fn old_sync(
        conn: &mut PgConn,
        connection_id: Uuid,
        account_id: Uuid,
        ago: Span,
    ) -> anyhow::Result<WorkspaceConnectionSync> {
        let sync = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;
        backdate::sync_started_at(conn, sync.id, Timestamp::now() - ago).await?;
        Ok(sync)
    }

    #[tokio::test]
    async fn complete_transitions_active_and_is_a_noop_when_terminal() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, _ws, connection_id) = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        let sync = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;
        assert_eq!(sync.status, SyncStatus::Running);

        // Completing an active sync succeeds and records the count.
        let completed = conn
            .complete_workspace_connection_sync(sync.id, 42)
            .await?
            .expect("active sync should complete");
        assert_eq!(completed.status, SyncStatus::Completed);
        assert_eq!(completed.records_synced, 42);
        assert!(completed.completed_at.is_some());

        // A second completion is a guarded no-op: the terminal row is untouched.
        assert!(
            conn.complete_workspace_connection_sync(sync.id, 99)
                .await?
                .is_none()
        );
        let reread = conn
            .find_workspace_connection_sync_by_id(sync.id)
            .await?
            .expect("sync exists");
        assert_eq!(reread.records_synced, 42, "count must not be rewritten");
        Ok(())
    }

    #[tokio::test]
    async fn fail_and_cancel_respect_the_terminal_guard() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, _ws, connection_id) = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        // Fail an active sync, then confirm a later cancel is a no-op.
        let sync = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;
        let failed = conn
            .fail_workspace_connection_sync(sync.id, "boom")
            .await?
            .expect("active sync should fail");
        assert_eq!(failed.status, SyncStatus::Failed);
        assert_eq!(failed.error_message.as_deref(), Some("boom"));
        assert!(
            conn.cancel_workspace_connection_sync(sync.id)
                .await?
                .is_none(),
            "a failed sync cannot be cancelled"
        );

        // Cancel a fresh active sync; a later completion is then a no-op.
        let other = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;
        let cancelled = conn
            .cancel_workspace_connection_sync(other.id)
            .await?
            .expect("active sync should cancel");
        assert_eq!(cancelled.status, SyncStatus::Cancelled);
        assert!(
            conn.complete_workspace_connection_sync(other.id, 5)
                .await?
                .is_none(),
            "a cancelled sync cannot complete"
        );
        Ok(())
    }

    #[tokio::test]
    async fn find_in_workspace_is_scoped_through_the_connection() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, workspace_id, connection_id) = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        let sync = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;

        // Found through its owning connection's workspace.
        assert!(
            conn.find_connection_sync_in_workspace(workspace_id, sync.id)
                .await?
                .is_some()
        );
        // Not found when scoped to a different workspace.
        assert!(
            conn.find_connection_sync_in_workspace(Uuid::now_v7(), sync.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn last_successful_sync_at_counts_only_completed() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, _ws, connection_id) = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        // A completed sync sets the connection's "last synced" instant.
        let completed = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;
        let _ = conn
            .complete_workspace_connection_sync(completed.id, 1)
            .await?;

        // A later failed sync does NOT move it.
        let failed = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;
        let _ = conn
            .fail_workspace_connection_sync(failed.id, "nope")
            .await?;

        let result = conn.last_successful_sync_at(&[connection_id]).await?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, connection_id);

        // A connection that never completed a sync is simply absent.
        let (_a, _w, never) = seed_connection(&db).await?;
        assert!(conn.last_successful_sync_at(&[never]).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn find_latest_syncs_pick_the_newest_per_connection() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, _ws, connection_id) = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        // An older sync, then a newer one for the same connection. Only one
        // sync may be active per connection, so the older one is completed
        // before the newer starts (as it would be in the real lifecycle).
        let older = old_sync(&mut conn, connection_id, account_id, Span::new().hours(2)).await?;
        let _ = conn.complete_workspace_connection_sync(older.id, 0).await?;
        let newer = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;

        // The single-connection latest is the newest.
        let latest = conn
            .find_latest_workspace_connection_sync(connection_id)
            .await?;
        assert_eq!(latest.map(|s| s.id), Some(newer.id));

        // The batched latest returns exactly one row (the newest) per connection.
        let batched = conn
            .find_latest_workspace_connection_syncs(&[connection_id])
            .await?;
        assert_eq!(
            batched.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![newer.id]
        );
        Ok(())
    }

    #[tokio::test]
    async fn fail_stale_running_syncs_reaps_only_old_running_ones() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let mut conn = db.client.get_connection().await?;

        // Only one active sync is allowed per connection, so each active sync
        // below lives on its own connection.

        // An old Running sync (should be reaped).
        let (stale_acct, _w1, stale_conn) = seed_connection(&db).await?;
        let stale = old_sync(&mut conn, stale_conn, stale_acct, Span::new().hours(2)).await?;

        // A recent Running sync (too new to reap).
        let (fresh_acct, _w2, fresh_conn) = seed_connection(&db).await?;
        let fresh = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                fresh_conn, fresh_acct,
            ))
            .await?;

        // An old but already-completed sync (not Running, so left alone).
        let (done_acct, _w3, done_conn) = seed_connection(&db).await?;
        let done = old_sync(&mut conn, done_conn, done_acct, Span::new().hours(2)).await?;
        let _ = conn.complete_workspace_connection_sync(done.id, 0).await?;

        let cutoff = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(1));
        assert_eq!(conn.fail_stale_running_syncs(cutoff).await?, 1);

        // Only the stale Running one flipped to Failed.
        let stale = conn
            .find_workspace_connection_sync_by_id(stale.id)
            .await?
            .expect("exists");
        assert_eq!(stale.status, SyncStatus::Failed);
        let fresh = conn
            .find_workspace_connection_sync_by_id(fresh.id)
            .await?
            .expect("exists");
        assert_eq!(fresh.status, SyncStatus::Running);
        let done = conn
            .find_workspace_connection_sync_by_id(done.id)
            .await?
            .expect("exists");
        assert_eq!(done.status, SyncStatus::Completed);
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_applies_status_filter() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let (account_id, _ws, connection_id) = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        let completed = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;
        let _ = conn
            .complete_workspace_connection_sync(completed.id, 1)
            .await?;
        let running = conn
            .create_workspace_connection_sync(NewWorkspaceConnectionSync::test(
                connection_id,
                account_id,
            ))
            .await?;

        // Filtering to Running returns only the active run.
        let page = conn
            .cursor_list_workspace_connection_syncs(
                connection_id,
                CursorPagination::new(50),
                Some(SyncStatus::Running),
            )
            .await?;
        assert_eq!(
            page.items.iter().map(|s| s.item.id).collect::<Vec<_>>(),
            vec![running.id]
        );
        Ok(())
    }
}
