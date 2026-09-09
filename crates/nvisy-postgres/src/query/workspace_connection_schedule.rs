//! Repository for a connection's sync schedule (the sync capability's config).

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceConnectionSchedule, WorkspaceConnectionSchedule};
use crate::{Error, PgConnection, Result, schema};

/// Repository for connection sync-schedule operations.
///
/// A schedule row is a connection's scheduled-sync config, present for
/// connections that sync on a timer. Transfer capability is the connection's
/// `provider_type`, not this row's presence.
pub trait WorkspaceConnectionScheduleRepository {
    /// Inserts a connection's sync schedule.
    fn create_connection_schedule(
        &mut self,
        schedule: NewWorkspaceConnectionSchedule,
    ) -> impl Future<Output = Result<WorkspaceConnectionSchedule>> + Send;

    /// Inserts or replaces a connection's sync schedule.
    ///
    /// Used when updating a scheduled connection: the schedule row may or may not
    /// already exist, so this upserts rather than assuming one is present.
    fn upsert_connection_schedule(
        &mut self,
        schedule: NewWorkspaceConnectionSchedule,
    ) -> impl Future<Output = Result<WorkspaceConnectionSchedule>> + Send;

    /// Finds a connection's sync schedule, if it has one.
    fn find_connection_schedule(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceConnectionSchedule>>> + Send;

    /// Finds the sync schedules for a set of connections in one query.
    ///
    /// Only connections with a schedule are returned; the rest are simply absent.
    /// Lets a page of connections resolve its schedules in a single round-trip.
    fn find_schedules(
        &mut self,
        connection_ids: &[Uuid],
    ) -> impl Future<Output = Result<Vec<WorkspaceConnectionSchedule>>> + Send;
}

impl WorkspaceConnectionScheduleRepository for PgConnection {
    async fn create_connection_schedule(
        &mut self,
        schedule: NewWorkspaceConnectionSchedule,
    ) -> Result<WorkspaceConnectionSchedule> {
        use schema::workspace_connection_schedule;

        let schedule = diesel::insert_into(workspace_connection_schedule::table)
            .values(&schedule)
            .returning(WorkspaceConnectionSchedule::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(schedule)
    }

    async fn upsert_connection_schedule(
        &mut self,
        schedule: NewWorkspaceConnectionSchedule,
    ) -> Result<WorkspaceConnectionSchedule> {
        use schema::workspace_connection_schedule::{self, dsl};

        let schedule = diesel::insert_into(workspace_connection_schedule::table)
            .values(&schedule)
            .on_conflict(dsl::connection_id)
            .do_update()
            .set((
                dsl::sync_mode.eq(diesel::upsert::excluded(dsl::sync_mode)),
                dsl::schedule_cron.eq(diesel::upsert::excluded(dsl::schedule_cron)),
                dsl::deletion_policy.eq(diesel::upsert::excluded(dsl::deletion_policy)),
            ))
            .returning(WorkspaceConnectionSchedule::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(schedule)
    }

    async fn find_connection_schedule(
        &mut self,
        connection_id: Uuid,
    ) -> Result<Option<WorkspaceConnectionSchedule>> {
        use schema::workspace_connection_schedule::{self, dsl};

        let schedule = workspace_connection_schedule::table
            .filter(dsl::connection_id.eq(connection_id))
            .select(WorkspaceConnectionSchedule::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(schedule)
    }

    async fn find_schedules(
        &mut self,
        connection_ids: &[Uuid],
    ) -> Result<Vec<WorkspaceConnectionSchedule>> {
        use schema::workspace_connection_schedule::{self, dsl};

        if connection_ids.is_empty() {
            return Ok(Vec::new());
        }

        let schedules = workspace_connection_schedule::table
            .filter(dsl::connection_id.eq_any(connection_ids))
            .select(WorkspaceConnectionSchedule::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(schedules)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::model::{NewWorkspaceConnection, NewWorkspaceConnectionSchedule};
    use crate::query::{WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository};
    use crate::test_util::TestDatabase;
    use crate::types::{SyncDeletionPolicy, SyncMode};

    /// Seeds a connection in the fixture's workspace and returns its id — the FK
    /// parent a schedule row requires.
    async fn seed_connection(db: &TestDatabase) -> anyhow::Result<Uuid> {
        let (account_id, workspace_id) = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let connection = conn
            .create_workspace_connection(NewWorkspaceConnection::test(workspace_id, account_id))
            .await?;
        Ok(connection.id)
    }

    #[tokio::test]
    async fn create_applies_database_defaults() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let connection_id = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        let schedule = conn
            .create_connection_schedule(NewWorkspaceConnectionSchedule::test(connection_id))
            .await?;

        // The mode and deletion policy default; there is no cron (manual-only).
        assert_eq!(schedule.sync_mode, SyncMode::Import);
        assert_eq!(schedule.deletion_policy, SyncDeletionPolicy::Ignore);
        assert!(schedule.schedule_cron.is_none());

        let found = conn.find_connection_schedule(connection_id).await?;
        assert_eq!(found.map(|s| s.connection_id), Some(connection_id));
        Ok(())
    }

    #[tokio::test]
    async fn upsert_replaces_an_existing_schedule() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let connection_id = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        // First upsert inserts an import schedule with a cron.
        let mut first = NewWorkspaceConnectionSchedule::test(connection_id);
        first.schedule_cron = Some("0 * * * *".to_owned());
        let first = conn.upsert_connection_schedule(first).await?;
        assert_eq!(first.sync_mode, SyncMode::Import);

        // Second upsert on the same connection replaces every field.
        let mut second = NewWorkspaceConnectionSchedule::test(connection_id);
        second.sync_mode = Some(SyncMode::Export);
        second.schedule_cron = None;
        second.deletion_policy = Some(SyncDeletionPolicy::Delete);
        let second = conn.upsert_connection_schedule(second).await?;
        assert_eq!(second.sync_mode, SyncMode::Export);
        assert_eq!(second.deletion_policy, SyncDeletionPolicy::Delete);
        assert!(second.schedule_cron.is_none());

        // Still exactly one row for the connection.
        let all = conn.find_schedules(&[connection_id]).await?;
        assert_eq!(all.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn find_schedules_returns_only_the_present_ones() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let scheduled = seed_connection(&db).await?;
        let unscheduled = seed_connection(&db).await?;
        let mut conn = db.client.get_connection().await?;

        let _ = conn
            .create_connection_schedule(NewWorkspaceConnectionSchedule::test(scheduled))
            .await?;

        // The batch query returns only the connection that has a schedule.
        let found = conn
            .find_schedules(&[scheduled, unscheduled, Uuid::now_v7()])
            .await?;
        assert_eq!(
            found.iter().map(|s| s.connection_id).collect::<Vec<_>>(),
            vec![scheduled]
        );

        // An empty input is a no-op, not a full-table scan.
        assert!(conn.find_schedules(&[]).await?.is_empty());
        Ok(())
    }
}
