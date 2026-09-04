//! Repository for a connection's sync schedule (the sync capability's config).

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceConnectionSchedule, WorkspaceConnectionSchedule};
use crate::{Error, PgConnection, Result, schema};

/// Repository for connection sync-schedule operations.
///
/// A schedule row exists only for sync-capable connections; its presence marks
/// a connection as sync-capable.
pub trait WorkspaceConnectionScheduleRepository {
    /// Inserts a connection's sync schedule.
    fn create_connection_schedule(
        &mut self,
        schedule: NewWorkspaceConnectionSchedule,
    ) -> impl Future<Output = Result<WorkspaceConnectionSchedule>> + Send;

    /// Inserts or replaces a connection's sync schedule.
    ///
    /// Used when updating a sync-capable connection: the schedule row may or may
    /// not already exist, so this upserts rather than assuming one is present.
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
    /// Only sync-capable connections are returned; the rest are simply absent.
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
