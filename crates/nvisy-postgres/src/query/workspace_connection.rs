//! Workspace connections repository for managing encrypted provider connections.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceConnection, UpdateWorkspaceConnection, WorkspaceConnection};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, OffsetPagination, ProviderType, SyncMode,
    WithAccountRef,
};
use crate::{Error, PgConnection, Result, schema};

/// A sync-scheduled connection paired with its cron expression, as returned by
/// [`WorkspaceConnectionRepository::list_scheduled_connections`]. The cron is
/// non-optional: the query only lists connections whose schedule has one.
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

    /// Finds a connection by its unique identifier.
    fn find_workspace_connection_by_id(
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

    /// Finds connections by provider type within a workspace.
    fn find_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> impl Future<Output = Result<Vec<WorkspaceConnection>>> + Send;

    /// Finds the workspace's most recently updated live, enabled connection of a
    /// given capability (e.g. its language model), if any. Resolves a capability
    /// connection without decrypting every connection's config.
    ///
    /// Disabled (`is_active = false`) connections are excluded: a disabled
    /// connection is not usable, and a newer disabled one must not shadow an
    /// active one.
    fn find_connection_by_type(
        &mut self,
        workspace_id: Uuid,
        provider_type: ProviderType,
    ) -> impl Future<Output = Result<Option<WorkspaceConnection>>> + Send;

    /// Lists all active, import-mode connections that have a sync schedule,
    /// across every workspace, each paired with its cron. Used by the
    /// scheduled-sync worker; returning the cron avoids re-reading each
    /// schedule row.
    fn list_scheduled_connections(
        &mut self,
    ) -> impl Future<Output = Result<Vec<ScheduledConnection>>> + Send;

    /// Lists all connections in a workspace with offset pagination.
    fn offset_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<WorkspaceConnection>>> + Send;

    /// Lists all connections in a workspace with cursor pagination, each paired
    /// with the handle and avatar of the account that created it.
    ///
    /// An empty `providers` slice means no provider filter; otherwise a
    /// connection matches if its provider is any of the given ones.
    fn cursor_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
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

    /// Counts connections in a workspace.
    fn count_workspace_connections(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = Result<i64>> + Send;

    /// Counts connections by provider in a workspace.
    fn count_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> impl Future<Output = Result<i64>> + Send;
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

    async fn find_workspace_connection_by_id(
        &mut self,
        connection_id: Uuid,
    ) -> Result<Option<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connection = workspace_connections::table
            .filter(dsl::id.eq(connection_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceConnection::as_select())
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

    async fn find_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> Result<Vec<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connections = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::provider.eq(provider))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::display_name.asc())
            .select(WorkspaceConnection::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(connections)
    }

    async fn find_connection_by_type(
        &mut self,
        workspace_id: Uuid,
        provider_type: ProviderType,
    ) -> Result<Option<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::provider_type.eq(provider_type))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::is_active.eq(true))
            .order(dsl::updated_at.desc())
            .select(WorkspaceConnection::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_scheduled_connections(&mut self) -> Result<Vec<ScheduledConnection>> {
        use schema::workspace_connection_schedule as sched;
        use schema::workspace_connections::{self, dsl};

        // Sync config lives in the schedule satellite; join it to find active,
        // import-mode connections with a cron schedule. The `schedule_cron IS NOT
        // NULL` filter makes the column non-null for this query, so the worker
        // gets the cron without re-reading the schedule row.
        let connections = workspace_connections::table
            .inner_join(sched::table.on(sched::connection_id.eq(dsl::id)))
            .filter(sched::schedule_cron.is_not_null())
            .filter(sched::sync_mode.eq(SyncMode::Import))
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

    async fn offset_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> Result<Vec<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connections = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(WorkspaceConnection::as_select())
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(connections)
    }

    async fn cursor_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        providers: &[String],
    ) -> Result<CursorPage<WithAccountRef<WorkspaceConnection>>> {
        use schema::workspace_connections::dsl;
        use schema::{accounts, workspace_connections};

        // Build base query with filters
        let mut base_query = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply provider filter (any-of)
        if !providers.is_empty() {
            base_query = base_query.filter(dsl::provider.eq_any(providers.to_vec()));
        }

        let total = if pagination.include_count {
            Some(
                base_query
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        // Rebuild query for fetching items
        let mut query = workspace_connections::table
            .inner_join(accounts::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if !providers.is_empty() {
            query = query.filter(dsl::provider.eq_any(providers.to_vec()));
        }

        let limit = pagination.fetch_limit();

        let rows: Vec<(WorkspaceConnection, AccountRefRow)> =
            if let Some(cursor) = &pagination.after {
                let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

                query
                    .filter(
                        dsl::created_at
                            .lt(&cursor_time)
                            .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                    )
                    .select((
                        WorkspaceConnection::as_select(),
                        (
                            accounts::username,
                            accounts::display_name,
                            accounts::avatar_url,
                        ),
                    ))
                    .order((dsl::created_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(Error::from)?
            } else {
                query
                    .select((
                        WorkspaceConnection::as_select(),
                        (
                            accounts::username,
                            accounts::display_name,
                            accounts::avatar_url,
                        ),
                    ))
                    .order((dsl::created_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(Error::from)?
            };

        let items: Vec<WithAccountRef<WorkspaceConnection>> = rows
            .into_iter()
            .map(|(item, account)| WithAccountRef { item, account })
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |wc| {
            (wc.item.created_at.into(), wc.item.id)
        }))
    }

    async fn update_workspace_connection(
        &mut self,
        connection_id: Uuid,
        updates: UpdateWorkspaceConnection,
    ) -> Result<WorkspaceConnection> {
        use schema::workspace_connections::{self, dsl};

        let connection =
            diesel::update(workspace_connections::table.filter(dsl::id.eq(connection_id)))
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

        diesel::update(workspace_connections::table.filter(dsl::id.eq(connection_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn count_workspace_connections(&mut self, workspace_id: Uuid) -> Result<i64> {
        use schema::workspace_connections::{self, dsl};

        let count = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(count)
    }

    async fn count_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> Result<i64> {
        use schema::workspace_connections::{self, dsl};

        let count = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::provider.eq(provider))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(count)
    }
}
