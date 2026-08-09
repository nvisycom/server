//! Repository for a connection's sync schedule (the sync capability's config).

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceConnectionSchedule, UpdateWorkspaceConnectionSchedule, WorkspaceConnectionSchedule,
};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for connection sync-schedule operations.
///
/// A schedule row exists only for sync-capable connections; its presence marks
/// a connection as sync-capable.
pub trait WorkspaceConnectionScheduleRepository {
    /// Inserts a connection's sync schedule.
    fn create_connection_schedule(
        &mut self,
        schedule: NewWorkspaceConnectionSchedule,
    ) -> impl Future<Output = PgResult<WorkspaceConnectionSchedule>> + Send;

    /// Finds a connection's sync schedule, if it has one.
    fn find_connection_schedule(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnectionSchedule>>> + Send;

    /// Updates a connection's sync schedule.
    fn update_connection_schedule(
        &mut self,
        connection_id: Uuid,
        updates: UpdateWorkspaceConnectionSchedule,
    ) -> impl Future<Output = PgResult<WorkspaceConnectionSchedule>> + Send;
}

impl WorkspaceConnectionScheduleRepository for PgConnection {
    async fn create_connection_schedule(
        &mut self,
        schedule: NewWorkspaceConnectionSchedule,
    ) -> PgResult<WorkspaceConnectionSchedule> {
        use schema::workspace_connection_schedule;

        let schedule = diesel::insert_into(workspace_connection_schedule::table)
            .values(&schedule)
            .returning(WorkspaceConnectionSchedule::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(schedule)
    }

    async fn find_connection_schedule(
        &mut self,
        connection_id: Uuid,
    ) -> PgResult<Option<WorkspaceConnectionSchedule>> {
        use schema::workspace_connection_schedule::{self, dsl};

        let schedule = workspace_connection_schedule::table
            .filter(dsl::connection_id.eq(connection_id))
            .select(WorkspaceConnectionSchedule::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(schedule)
    }

    async fn update_connection_schedule(
        &mut self,
        connection_id: Uuid,
        updates: UpdateWorkspaceConnectionSchedule,
    ) -> PgResult<WorkspaceConnectionSchedule> {
        use schema::workspace_connection_schedule::{self, dsl};

        let schedule = diesel::update(
            workspace_connection_schedule::table.filter(dsl::connection_id.eq(connection_id)),
        )
        .set(&updates)
        .returning(WorkspaceConnectionSchedule::as_returning())
        .get_result(self)
        .await
        .map_err(PgError::from)?;

        Ok(schedule)
    }
}
