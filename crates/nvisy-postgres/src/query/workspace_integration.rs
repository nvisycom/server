//! Workspace integration repository for managing workspace integration operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceIntegration, UpdateWorkspaceIntegration, WorkspaceIntegration};
use crate::types::{CursorPage, CursorPagination, IntegrationStatus, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace integration database operations.
///
/// Handles third-party integration management including CRUD operations, status tracking,
/// and integration lifecycle management.
pub trait WorkspaceIntegrationRepository {
    /// Creates a new workspace integration with the provided configuration.
    fn create_workspace_integration(
        &mut self,
        integration: NewWorkspaceIntegration,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Finds a workspace integration by its unique identifier.
    fn find_workspace_integration_by_id(
        &mut self,
        integration_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceIntegration>>> + Send;

    /// Updates an existing workspace integration with new configuration or status.
    fn update_workspace_integration(
        &mut self,
        integration_id: Uuid,
        changes: UpdateWorkspaceIntegration,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Soft deletes a workspace integration by deactivating it.
    fn delete_workspace_integration(
        &mut self,
        integration_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists workspace integrations for a workspace with offset pagination.
    fn offset_list_workspace_integrations(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Lists workspace integrations for a workspace with cursor pagination.
    fn cursor_list_workspace_integrations(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<WorkspaceIntegration>>> + Send;

    /// Updates the sync status of a workspace integration.
    fn update_workspace_integration_status(
        &mut self,
        integration_id: Uuid,
        new_status: IntegrationStatus,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;
}

impl WorkspaceIntegrationRepository for PgConnection {
    async fn create_workspace_integration(
        &mut self,
        integration: NewWorkspaceIntegration,
    ) -> PgResult<WorkspaceIntegration> {
        use schema::workspace_integrations;

        let integration = diesel::insert_into(workspace_integrations::table)
            .values(&integration)
            .returning(WorkspaceIntegration::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn find_workspace_integration_by_id(
        &mut self,
        integration_id: Uuid,
    ) -> PgResult<Option<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let integration = workspace_integrations
            .filter(id.eq(integration_id))
            .select(WorkspaceIntegration::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn update_workspace_integration(
        &mut self,
        integration_id: Uuid,
        changes: UpdateWorkspaceIntegration,
    ) -> PgResult<WorkspaceIntegration> {
        use schema::workspace_integrations::dsl::*;

        let integration = diesel::update(workspace_integrations)
            .filter(id.eq(integration_id))
            .set(&changes)
            .returning(WorkspaceIntegration::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn delete_workspace_integration(&mut self, integration_id: Uuid) -> PgResult<()> {
        use schema::workspace_integrations::dsl::*;

        diesel::update(workspace_integrations)
            .filter(id.eq(integration_id))
            .set(is_active.eq(false))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn offset_list_workspace_integrations(
        &mut self,
        workspace_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::{self, dsl};

        let integrations = workspace_integrations::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .select(WorkspaceIntegration::as_select())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn cursor_list_workspace_integrations(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<WorkspaceIntegration>> {
        use schema::workspace_integrations::{self, dsl};

        let total = if pagination.include_count {
            Some(
                workspace_integrations::table
                    .filter(dsl::workspace_id.eq(workspace_id))
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let limit = pagination.limit + 1;

        let items: Vec<WorkspaceIntegration> = if let Some(cursor) = &pagination.after {
            let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

            workspace_integrations::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(
                    dsl::created_at
                        .lt(&cursor_time)
                        .or(dsl::created_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                )
                .select(WorkspaceIntegration::as_select())
                .order((dsl::created_at.desc(), dsl::id.desc()))
                .limit(limit)
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            workspace_integrations::table
                .filter(dsl::workspace_id.eq(workspace_id))
                .select(WorkspaceIntegration::as_select())
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
            |i: &WorkspaceIntegration| (i.created_at.into(), i.id),
        ))
    }

    async fn update_workspace_integration_status(
        &mut self,
        integration_id: Uuid,
        new_status: IntegrationStatus,
    ) -> PgResult<WorkspaceIntegration> {
        let changes = UpdateWorkspaceIntegration {
            sync_status: Some(Some(new_status)),
            ..Default::default()
        };

        self.update_workspace_integration(integration_id, changes)
            .await
    }
}
