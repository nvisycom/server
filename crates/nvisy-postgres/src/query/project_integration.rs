//! Project integration repository for managing project integration operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectIntegration, ProjectIntegration, UpdateProjectIntegration};
use crate::types::IntegrationStatus;
use crate::{PgError, PgResult, schema};

/// Repository for project integration table operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectIntegrationRepository;

impl ProjectIntegrationRepository {
    /// Creates a new project integration repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new project integration.
    pub async fn create_integration(
        conn: &mut AsyncPgConnection,
        integration: NewProjectIntegration,
    ) -> PgResult<ProjectIntegration> {
        use schema::project_integrations;

        let integration = diesel::insert_into(project_integrations::table)
            .values(&integration)
            .returning(ProjectIntegration::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integration)
    }

    /// Finds an integration by its ID.
    pub async fn find_integration_by_id(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
    ) -> PgResult<Option<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integration = project_integrations
            .filter(id.eq(integration_id))
            .filter(deleted_at.is_null())
            .select(ProjectIntegration::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    /// Updates an integration.
    pub async fn update_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        changes: UpdateProjectIntegration,
    ) -> PgResult<ProjectIntegration> {
        use schema::project_integrations::dsl::*;

        let integration = diesel::update(project_integrations)
            .filter(id.eq(integration_id))
            .filter(deleted_at.is_null())
            .set(&changes)
            .returning(ProjectIntegration::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integration)
    }

    /// Soft deletes an integration.
    pub async fn delete_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
    ) -> PgResult<()> {
        use schema::project_integrations::dsl::*;

        diesel::update(project_integrations)
            .filter(id.eq(integration_id))
            .filter(deleted_at.is_null())
            .set(deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Lists all integrations for a project.
    pub async fn list_project_integrations(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(deleted_at.is_null())
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Finds integrations by their status.
    pub async fn find_integrations_by_status(
        conn: &mut AsyncPgConnection,
        integration_status: IntegrationStatus,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(status.eq(integration_status))
            .filter(deleted_at.is_null())
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Finds an integration by project ID and integration name.
    pub async fn find_integration_by_project_and_name(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        name: &str,
    ) -> PgResult<Option<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integration = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(integration_name.eq(name))
            .filter(deleted_at.is_null())
            .select(ProjectIntegration::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    /// Enables an integration.
    pub async fn enable_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            is_enabled: Some(true),
            updated_by: Some(updated_by),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Disables an integration.
    pub async fn disable_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            is_enabled: Some(false),
            updated_by: Some(updated_by),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Lists only active integrations for a project.
    pub async fn list_active_integrations(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(is_enabled.eq(true))
            .filter(deleted_at.is_null())
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Updates integration status.
    pub async fn update_integration_status(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        new_status: IntegrationStatus,
        updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            status: Some(new_status),
            updated_by: Some(updated_by),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Lists integrations that need attention (have errors or are disabled).
    pub async fn list_integrations_needing_attention(
        conn: &mut AsyncPgConnection,
        proj_id: Option<Uuid>,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut query = project_integrations
            .filter(deleted_at.is_null())
            .into_boxed();

        if let Some(pid) = proj_id {
            query = query.filter(project_id.eq(pid));
        }

        let integrations = query
            .filter(
                status
                    .eq(IntegrationStatus::Failure)
                    .or(is_enabled.eq(false)),
            )
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Gets integration statistics for a project.
    pub async fn get_integration_stats(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<(i64, i64, i64, i64)> {
        use schema::project_integrations::dsl::*;

        // Count total integrations
        let total_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count active integrations
        let active_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(is_enabled.eq(true))
            .filter(status.eq(IntegrationStatus::Executing))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count failed integrations
        let failed_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(status.eq(IntegrationStatus::Failure))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count pending integrations
        let pending_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(status.eq(IntegrationStatus::Pending))
            .filter(deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok((total_count, active_count, failed_count, pending_count))
    }

    /// Updates integration authentication data.
    pub async fn update_integration_auth(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        auth_data: serde_json::Value,
        updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            auth_data: Some(auth_data),
            updated_by: Some(updated_by),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Updates integration metadata.
    pub async fn update_integration_metadata(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        metadata: serde_json::Value,
        updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            metadata: Some(metadata),
            updated_by: Some(updated_by),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Lists integrations created by a specific user.
    pub async fn list_integrations_by_creator(
        conn: &mut AsyncPgConnection,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(created_by.eq(creator_id))
            .filter(deleted_at.is_null())
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Finds integrations by name pattern across projects.
    pub async fn find_integrations_by_name_pattern(
        conn: &mut AsyncPgConnection,
        name_pattern: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let search_pattern = format!("%{}%", name_pattern);

        let integrations = project_integrations
            .filter(integration_name.ilike(&search_pattern))
            .filter(deleted_at.is_null())
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Gets recently updated integrations.
    pub async fn get_recently_updated_integrations(
        conn: &mut AsyncPgConnection,
        proj_id: Option<Uuid>,
        hours: i64,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(hours);

        let mut query = project_integrations
            .filter(updated_at.gt(cutoff_time))
            .filter(deleted_at.is_null())
            .into_boxed();

        if let Some(pid) = proj_id {
            query = query.filter(project_id.eq(pid));
        }

        let integrations = query
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .limit(50)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Checks if an integration name is unique within a project.
    pub async fn is_integration_name_unique(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> PgResult<bool> {
        use schema::project_integrations::dsl::*;

        let mut query = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(integration_name.eq(name))
            .filter(deleted_at.is_null())
            .into_boxed();

        if let Some(exclude) = exclude_id {
            query = query.filter(id.ne(exclude));
        }

        let count: i64 = query
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count == 0)
    }
}
