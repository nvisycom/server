//! Workspace integration repository for managing workspace integration operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewWorkspaceIntegration, UpdateWorkspaceIntegration, WorkspaceIntegration};
use crate::types::{IntegrationFilter, IntegrationStatus};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace integration database operations.
///
/// Handles third-party integration management including CRUD operations, status tracking,
/// and integration lifecycle management.
pub trait WorkspaceIntegrationRepository {
    /// Creates a new workspace integration with the provided configuration.
    fn create_integration(
        &mut self,
        integration: NewWorkspaceIntegration,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Finds an integration by its unique identifier.
    fn find_integration_by_id(
        &mut self,
        integration_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<WorkspaceIntegration>>> + Send;

    /// Updates an existing integration with new configuration or status.
    fn update_integration(
        &mut self,
        integration_id: Uuid,
        changes: UpdateWorkspaceIntegration,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Soft deletes an integration by deactivating it.
    fn delete_integration(
        &mut self,
        integration_id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Lists all integrations for a specific workspace.
    fn list_workspace_integrations(
        &mut self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Lists integrations for a workspace with filtering options.
    ///
    /// Supports filtering by integration type.
    fn list_workspace_integrations_filtered(
        &mut self,
        proj_id: Uuid,
        filter: IntegrationFilter,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Finds all integrations matching a specific sync status.
    fn find_integrations_by_status(
        &mut self,
        integration_status: IntegrationStatus,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Finds an integration by workspace and name combination.
    fn find_integration_by_workspace_and_name(
        &mut self,
        proj_id: Uuid,
        name: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspaceIntegration>>> + Send;

    /// Enables an integration for active operation.
    fn enable_integration(
        &mut self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Disables an integration to stop all operations.
    fn disable_integration(
        &mut self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Lists only active integrations for a specific workspace.
    fn list_active_integrations(
        &mut self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Updates the sync status of an integration.
    fn update_integration_status(
        &mut self,
        integration_id: Uuid,
        new_status: IntegrationStatus,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Lists integrations that require administrator attention.
    fn list_integrations_needing_attention(
        &mut self,
        proj_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Updates the authentication credentials for an integration.
    fn update_integration_auth(
        &mut self,
        integration_id: Uuid,
        auth_data: serde_json::Value,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Updates the configuration metadata for an integration.
    fn update_integration_metadata(
        &mut self,
        integration_id: Uuid,
        metadata: serde_json::Value,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<WorkspaceIntegration>> + Send;

    /// Lists integrations created by a specific user with pagination.
    fn list_integrations_by_creator(
        &mut self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Finds integrations matching a name pattern across all workspaces.
    fn find_integrations_by_name_pattern(
        &mut self,
        name_pattern: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Gets integrations that have been recently updated within a time window.
    fn get_recently_updated_integrations(
        &mut self,
        proj_id: Option<Uuid>,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceIntegration>>> + Send;

    /// Checks if an integration name is unique within a workspace.
    fn is_integration_name_unique(
        &mut self,
        proj_id: Uuid,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl WorkspaceIntegrationRepository for PgConnection {
    async fn create_integration(
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

    async fn find_integration_by_id(
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

    async fn update_integration(
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

    async fn delete_integration(&mut self, integration_id: Uuid) -> PgResult<()> {
        use schema::workspace_integrations::dsl::*;

        diesel::update(workspace_integrations)
            .filter(id.eq(integration_id))
            .set(is_active.eq(false))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_workspace_integrations(
        &mut self,
        proj_id: Uuid,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let integrations = workspace_integrations
            .filter(workspace_id.eq(proj_id))
            .select(WorkspaceIntegration::as_select())
            .order(created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn list_workspace_integrations_filtered(
        &mut self,
        proj_id: Uuid,
        filter: IntegrationFilter,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let mut query = workspace_integrations
            .filter(workspace_id.eq(proj_id))
            .into_boxed();

        // Apply integration type filter
        if let Some(int_type) = filter.integration_type {
            query = query.filter(integration_type.eq(int_type));
        }

        let integrations = query
            .select(WorkspaceIntegration::as_select())
            .order(created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn find_integrations_by_status(
        &mut self,
        integration_status: IntegrationStatus,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let integrations = workspace_integrations
            .filter(sync_status.eq(Some(integration_status)))
            .select(WorkspaceIntegration::as_select())
            .order(created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn find_integration_by_workspace_and_name(
        &mut self,
        proj_id: Uuid,
        name: &str,
    ) -> PgResult<Option<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let integration = workspace_integrations
            .filter(workspace_id.eq(proj_id))
            .filter(integration_name.eq(name))
            .select(WorkspaceIntegration::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn enable_integration(
        &mut self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> PgResult<WorkspaceIntegration> {
        let changes = UpdateWorkspaceIntegration {
            is_active: Some(true),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn disable_integration(
        &mut self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> PgResult<WorkspaceIntegration> {
        let changes = UpdateWorkspaceIntegration {
            is_active: Some(false),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn list_active_integrations(
        &mut self,
        proj_id: Uuid,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let integrations = workspace_integrations
            .filter(workspace_id.eq(proj_id))
            .filter(is_active.eq(true))
            .select(WorkspaceIntegration::as_select())
            .order(created_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn update_integration_status(
        &mut self,
        integration_id: Uuid,
        new_status: IntegrationStatus,
        _updated_by: Uuid,
    ) -> PgResult<WorkspaceIntegration> {
        let changes = UpdateWorkspaceIntegration {
            sync_status: Some(Some(new_status)),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn list_integrations_needing_attention(
        &mut self,
        proj_id: Option<Uuid>,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let mut query = workspace_integrations.into_boxed();

        if let Some(pid) = proj_id {
            query = query.filter(workspace_id.eq(pid));
        }

        let integrations = query
            .filter(
                sync_status
                    .eq(Some(IntegrationStatus::Failed))
                    .or(is_active.eq(false)),
            )
            .select(WorkspaceIntegration::as_select())
            .order(updated_at.desc())
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn update_integration_auth(
        &mut self,
        integration_id: Uuid,
        auth_data: serde_json::Value,
        _updated_by: Uuid,
    ) -> PgResult<WorkspaceIntegration> {
        let changes = UpdateWorkspaceIntegration {
            credentials: Some(auth_data),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn update_integration_metadata(
        &mut self,
        integration_id: Uuid,
        metadata: serde_json::Value,
        _updated_by: Uuid,
    ) -> PgResult<WorkspaceIntegration> {
        let changes = UpdateWorkspaceIntegration {
            metadata: Some(metadata),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn list_integrations_by_creator(
        &mut self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let integrations = workspace_integrations
            .filter(created_by.eq(creator_id))
            .select(WorkspaceIntegration::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn find_integrations_by_name_pattern(
        &mut self,
        name_pattern: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let search_pattern = format!("%{}%", name_pattern);

        let integrations = workspace_integrations
            .filter(integration_name.ilike(&search_pattern))
            .select(WorkspaceIntegration::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn get_recently_updated_integrations(
        &mut self,
        proj_id: Option<Uuid>,
        hours: i64,
    ) -> PgResult<Vec<WorkspaceIntegration>> {
        use schema::workspace_integrations::dsl::*;

        let cutoff_time = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(hours));

        let mut query = workspace_integrations
            .filter(updated_at.gt(cutoff_time))
            .into_boxed();

        if let Some(pid) = proj_id {
            query = query.filter(workspace_id.eq(pid));
        }

        let integrations = query
            .select(WorkspaceIntegration::as_select())
            .order(updated_at.desc())
            .limit(50)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn is_integration_name_unique(
        &mut self,
        proj_id: Uuid,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> PgResult<bool> {
        use schema::workspace_integrations::dsl::*;

        let mut query = workspace_integrations
            .filter(workspace_id.eq(proj_id))
            .filter(integration_name.eq(name))
            .into_boxed();

        if let Some(exclude) = exclude_id {
            query = query.filter(id.ne(exclude));
        }

        let count: i64 = query
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count == 0)
    }
}
