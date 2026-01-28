//! Workspace connections repository for managing encrypted provider connections.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceConnection, UpdateWorkspaceConnection, WorkspaceConnection};
use crate::types::{CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace connection database operations.
///
/// Handles connection lifecycle management including creation, updates,
/// and workspace-scoped queries.
pub trait WorkspaceConnectionRepository {
    /// Creates a new workspace connection record.
    fn create_workspace_connection(
        &mut self,
        new_connection: NewWorkspaceConnection,
    ) -> impl Future<Output = PgResult<WorkspaceConnection>> + Send;

    /// Finds a connection by its unique identifier.
    fn find_workspace_connection_by_id(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnection>>> + Send;

    /// Finds a connection by ID within a specific workspace.
    ///
    /// Provides workspace-scoped access control at the database level.
    fn find_connection_in_workspace(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceConnection>>> + Send;

    /// Finds connections by provider type within a workspace.
    fn find_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceConnection>>> + Send;

    /// Lists all connections in a workspace with offset pagination.
    fn offset_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceConnection>>> + Send;

    /// Lists all connections in a workspace with cursor pagination.
    fn cursor_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        provider_filter: Option<&str>,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceConnection>>> + Send;

    /// Updates a connection with new data.
    fn update_workspace_connection(
        &mut self,
        connection_id: Uuid,
        updates: UpdateWorkspaceConnection,
    ) -> impl Future<Output = PgResult<WorkspaceConnection>> + Send;

    /// Soft deletes a connection by setting the deletion timestamp.
    fn delete_workspace_connection(
        &mut self,
        connection_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Counts connections in a workspace.
    fn count_workspace_connections(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Counts connections by provider in a workspace.
    fn count_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl WorkspaceConnectionRepository for PgConnection {
    async fn create_workspace_connection(
        &mut self,
        new_connection: NewWorkspaceConnection,
    ) -> PgResult<WorkspaceConnection> {
        use schema::workspace_connections;

        let connection = diesel::insert_into(workspace_connections::table)
            .values(&new_connection)
            .returning(WorkspaceConnection::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(connection)
    }

    async fn find_workspace_connection_by_id(
        &mut self,
        connection_id: Uuid,
    ) -> PgResult<Option<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connection = workspace_connections::table
            .filter(dsl::id.eq(connection_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceConnection::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(connection)
    }

    async fn find_connection_in_workspace(
        &mut self,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> PgResult<Option<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connection = workspace_connections::table
            .filter(dsl::id.eq(connection_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .select(WorkspaceConnection::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(connection)
    }

    async fn find_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> PgResult<Vec<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        let connections = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::provider.eq(provider))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::name.asc())
            .select(WorkspaceConnection::as_select())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(connections)
    }

    async fn offset_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceConnection>> {
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
            .map_err(PgError::from)?;

        Ok(connections)
    }

    async fn cursor_list_workspace_connections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        provider_filter: Option<&str>,
    ) -> PgResult<CursorPage<WorkspaceConnection>> {
        use schema::workspace_connections::{self, dsl};

        // Build base query with filters
        let mut base_query = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply provider filter
        if let Some(provider) = provider_filter {
            base_query = base_query.filter(dsl::provider.eq(provider));
        }

        let total = if pagination.include_count {
            Some(
                base_query
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Rebuild query for fetching items
        let mut query = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if let Some(provider) = provider_filter {
            query = query.filter(dsl::provider.eq(provider));
        }

        let limit = pagination.limit + 1;

        let items: Vec<WorkspaceConnection> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            query
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(WorkspaceConnection::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            query
                .select(WorkspaceConnection::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |c: &WorkspaceConnection| (c.created_at.into(), c.id),
        ))
    }

    async fn update_workspace_connection(
        &mut self,
        connection_id: Uuid,
        updates: UpdateWorkspaceConnection,
    ) -> PgResult<WorkspaceConnection> {
        use schema::workspace_connections::{self, dsl};

        let connection =
            diesel::update(workspace_connections::table.filter(dsl::id.eq(connection_id)))
                .set(&updates)
                .returning(WorkspaceConnection::as_returning())
                .get_result(self)
                .await
                .map_err(PgError::from)?;

        Ok(connection)
    }

    async fn delete_workspace_connection(&mut self, connection_id: Uuid) -> PgResult<()> {
        use diesel::dsl::now;
        use schema::workspace_connections::{self, dsl};

        diesel::update(workspace_connections::table.filter(dsl::id.eq(connection_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn count_workspace_connections(&mut self, workspace_id: Uuid) -> PgResult<i64> {
        use schema::workspace_connections::{self, dsl};

        let count = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }

    async fn count_workspace_connections_by_provider(
        &mut self,
        workspace_id: Uuid,
        provider: &str,
    ) -> PgResult<i64> {
        use schema::workspace_connections::{self, dsl};

        let count = workspace_connections::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::provider.eq(provider))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count)
    }
}
