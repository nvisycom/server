//! Project integration repository for managing project integration operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectIntegration, ProjectIntegration, UpdateProjectIntegration};
use crate::types::IntegrationStatus;
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for project integration database operations.
///
/// Handles third-party integration management including CRUD operations, status tracking,
/// and integration lifecycle management.
pub trait ProjectIntegrationRepository {
    /// Creates a new project integration with the provided configuration.
    fn create_integration(
        &self,
        integration: NewProjectIntegration,
    ) -> impl Future<Output = PgResult<ProjectIntegration>> + Send;

    /// Finds an integration by its unique identifier.
    fn find_integration_by_id(
        &self,
        integration_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<ProjectIntegration>>> + Send;

    /// Updates an existing integration with new configuration or status.
    fn update_integration(
        &self,
        integration_id: Uuid,
        changes: UpdateProjectIntegration,
    ) -> impl Future<Output = PgResult<ProjectIntegration>> + Send;

    /// Soft deletes an integration by deactivating it.
    fn delete_integration(&self, integration_id: Uuid)
    -> impl Future<Output = PgResult<()>> + Send;

    /// Lists all integrations for a specific project.
    fn list_project_integrations(
        &self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ProjectIntegration>>> + Send;

    /// Finds all integrations matching a specific sync status.
    fn find_integrations_by_status(
        &self,
        integration_status: IntegrationStatus,
    ) -> impl Future<Output = PgResult<Vec<ProjectIntegration>>> + Send;

    /// Finds an integration by project and name combination.
    fn find_integration_by_project_and_name(
        &self,
        proj_id: Uuid,
        name: &str,
    ) -> impl Future<Output = PgResult<Option<ProjectIntegration>>> + Send;

    /// Enables an integration for active operation.
    fn enable_integration(
        &self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<ProjectIntegration>> + Send;

    /// Disables an integration to stop all operations.
    fn disable_integration(
        &self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<ProjectIntegration>> + Send;

    /// Lists only active integrations for a specific project.
    fn list_active_integrations(
        &self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<ProjectIntegration>>> + Send;

    /// Updates the sync status of an integration.
    fn update_integration_status(
        &self,
        integration_id: Uuid,
        new_status: IntegrationStatus,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<ProjectIntegration>> + Send;

    /// Lists integrations that require administrator attention.
    fn list_integrations_needing_attention(
        &self,
        proj_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<Vec<ProjectIntegration>>> + Send;

    /// Gets comprehensive integration statistics for a project.
    fn get_integration_stats(
        &self,
        proj_id: Uuid,
    ) -> impl Future<Output = PgResult<(i64, i64, i64, i64)>> + Send;

    /// Updates the authentication credentials for an integration.
    fn update_integration_auth(
        &self,
        integration_id: Uuid,
        auth_data: serde_json::Value,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<ProjectIntegration>> + Send;

    /// Updates the configuration metadata for an integration.
    fn update_integration_metadata(
        &self,
        integration_id: Uuid,
        metadata: serde_json::Value,
        _updated_by: Uuid,
    ) -> impl Future<Output = PgResult<ProjectIntegration>> + Send;

    /// Lists integrations created by a specific user with pagination.
    fn list_integrations_by_creator(
        &self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectIntegration>>> + Send;

    /// Finds integrations matching a name pattern across all projects.
    fn find_integrations_by_name_pattern(
        &self,
        name_pattern: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<ProjectIntegration>>> + Send;

    /// Gets integrations that have been recently updated within a time window.
    fn get_recently_updated_integrations(
        &self,
        proj_id: Option<Uuid>,
        hours: i64,
    ) -> impl Future<Output = PgResult<Vec<ProjectIntegration>>> + Send;

    /// Checks if an integration name is unique within a project.
    fn is_integration_name_unique(
        &self,
        proj_id: Uuid,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl ProjectIntegrationRepository for PgClient {
    async fn create_integration(
        &self,
        integration: NewProjectIntegration,
    ) -> PgResult<ProjectIntegration> {
        use schema::project_integrations;

        let mut conn = self.get_connection().await?;

        let integration = diesel::insert_into(project_integrations::table)
            .values(&integration)
            .returning(ProjectIntegration::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn find_integration_by_id(
        &self,
        integration_id: Uuid,
    ) -> PgResult<Option<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let integration = project_integrations
            .filter(id.eq(integration_id))
            .select(ProjectIntegration::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn update_integration(
        &self,
        integration_id: Uuid,
        changes: UpdateProjectIntegration,
    ) -> PgResult<ProjectIntegration> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let integration = diesel::update(project_integrations)
            .filter(id.eq(integration_id))
            .set(&changes)
            .returning(ProjectIntegration::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn delete_integration(&self, integration_id: Uuid) -> PgResult<()> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        diesel::update(project_integrations)
            .filter(id.eq(integration_id))
            .set(is_active.eq(false))
            .execute(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_project_integrations(&self, proj_id: Uuid) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let integrations = project_integrations
            .filter(project_id.eq(proj_id))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn find_integrations_by_status(
        &self,
        integration_status: IntegrationStatus,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let integrations = project_integrations
            .filter(sync_status.eq(Some(integration_status)))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn find_integration_by_project_and_name(
        &self,
        proj_id: Uuid,
        name: &str,
    ) -> PgResult<Option<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let integration = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(integration_name.eq(name))
            .select(ProjectIntegration::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    async fn enable_integration(
        &self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            is_active: Some(true),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn disable_integration(
        &self,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            is_active: Some(false),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn list_active_integrations(&self, proj_id: Uuid) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let integrations = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn update_integration_status(
        &self,
        integration_id: Uuid,
        new_status: IntegrationStatus,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            sync_status: Some(new_status),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn list_integrations_needing_attention(
        &self,
        proj_id: Option<Uuid>,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let mut query = project_integrations.into_boxed();

        if let Some(pid) = proj_id {
            query = query.filter(project_id.eq(pid));
        }

        let integrations = query
            .filter(
                sync_status
                    .eq(Some(IntegrationStatus::Failed))
                    .or(is_active.eq(false)),
            )
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn get_integration_stats(&self, proj_id: Uuid) -> PgResult<(i64, i64, i64, i64)> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        // Count total integrations
        let total_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        // Count active integrations
        let active_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .filter(sync_status.eq(Some(IntegrationStatus::Executing)))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        // Count failed integrations
        let failed_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(sync_status.eq(Some(IntegrationStatus::Failed)))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        // Count pending integrations
        let pending_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(sync_status.eq(Some(IntegrationStatus::Pending)))
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok((total_count, active_count, failed_count, pending_count))
    }

    async fn update_integration_auth(
        &self,
        integration_id: Uuid,
        auth_data: serde_json::Value,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            credentials: Some(auth_data),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn update_integration_metadata(
        &self,
        integration_id: Uuid,
        metadata: serde_json::Value,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            metadata: Some(metadata),
            ..Default::default()
        };

        self.update_integration(integration_id, changes).await
    }

    async fn list_integrations_by_creator(
        &self,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let integrations = project_integrations
            .filter(created_by.eq(creator_id))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn find_integrations_by_name_pattern(
        &self,
        name_pattern: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let search_pattern = format!("%{}%", name_pattern);

        let integrations = project_integrations
            .filter(integration_name.ilike(&search_pattern))
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn get_recently_updated_integrations(
        &self,
        proj_id: Option<Uuid>,
        hours: i64,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let cutoff_time = jiff_diesel::Timestamp::from(Timestamp::now() - Span::new().hours(hours));

        let mut query = project_integrations
            .filter(updated_at.gt(cutoff_time))
            .into_boxed();

        if let Some(pid) = proj_id {
            query = query.filter(project_id.eq(pid));
        }

        let integrations = query
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .limit(50)
            .load(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    async fn is_integration_name_unique(
        &self,
        proj_id: Uuid,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> PgResult<bool> {
        use schema::project_integrations::dsl::*;

        let mut conn = self.get_connection().await?;

        let mut query = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(integration_name.eq(name))
            .into_boxed();

        if let Some(exclude) = exclude_id {
            query = query.filter(id.ne(exclude));
        }

        let count: i64 = query
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count == 0)
    }
}
