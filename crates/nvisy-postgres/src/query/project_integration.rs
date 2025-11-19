//! Project integration repository for managing project integration operations.
//!
//! This module provides comprehensive database operations for managing third-party
//! integrations connected to projects. It handles the full lifecycle of integrations
//! from creation and configuration through status monitoring, updates, and maintenance.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{NewProjectIntegration, ProjectIntegration, UpdateProjectIntegration};
use crate::types::IntegrationStatus;
use crate::{PgError, PgResult, schema};

/// Repository for project integration table operations.
///
/// Provides comprehensive database operations for managing project integrations,
/// including CRUD operations, status management, and analytical queries. This repository
/// handles all database interactions related to third-party service integrations.
///
/// The repository is stateless and thread-safe, designed to be used as a singleton
/// or instantiated as needed. All methods require an active database connection
/// and return results wrapped in the standard `PgResult` type for error handling.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProjectIntegrationRepository;

impl ProjectIntegrationRepository {
    /// Creates a new project integration repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `ProjectIntegrationRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new project integration with the provided configuration.
    ///
    /// Sets up a new integration between a project and an external service.
    /// The integration is created with the provided configuration and automatically
    /// assigned a unique ID and creation timestamp. The creator is recorded for
    /// attribution and permission management.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration` - Complete integration configuration data
    ///
    /// # Returns
    ///
    /// The created `ProjectIntegration` with database-generated ID and timestamps,
    /// or a database error if the operation fails.
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

    /// Finds an integration by its unique identifier.
    ///
    /// Retrieves a specific integration using its UUID. This is the primary
    /// method for accessing individual integrations when you know the exact ID,
    /// typically for update operations or detailed views.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `integration_id` - UUID of the integration to retrieve
    ///
    /// # Returns
    ///
    /// The matching `ProjectIntegration` if found, `None` if not found,
    /// or a database error if the query fails.
    pub async fn find_integration_by_id(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
    ) -> PgResult<Option<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integration = project_integrations
            .filter(id.eq(integration_id))
            .select(ProjectIntegration::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    /// Updates an existing integration with new configuration or status.
    ///
    /// Applies the specified changes to an integration using Diesel's changeset
    /// mechanism. Only the fields set to `Some(value)` in the update structure
    /// will be modified, while `None` fields remain unchanged. The updated_at
    /// timestamp is automatically updated.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration_id` - UUID of the integration to update
    /// * `changes` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `ProjectIntegration` with new values and timestamp,
    /// or a database error if the operation fails.
    pub async fn update_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        changes: UpdateProjectIntegration,
    ) -> PgResult<ProjectIntegration> {
        use schema::project_integrations::dsl::*;

        let integration = diesel::update(project_integrations)
            .filter(id.eq(integration_id))
            .set(&changes)
            .returning(ProjectIntegration::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integration)
    }

    /// Soft deletes an integration by deactivating it.
    ///
    /// Disables the integration rather than permanently deleting it from the database.
    /// This preserves the integration's configuration and history while preventing
    /// it from performing any sync operations or receiving webhooks.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration_id` - UUID of the integration to deactivate
    ///
    /// # Returns
    ///
    /// Unit type on success, or a database error if the operation fails.
    ///
    /// # Note
    ///
    /// This operation only sets `is_active` to false. The integration record
    /// and all its configuration remain in the database for potential reactivation
    /// or audit purposes.
    pub async fn delete_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
    ) -> PgResult<()> {
        use schema::project_integrations::dsl::*;

        diesel::update(project_integrations)
            .filter(id.eq(integration_id))
            .set(is_active.eq(false))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Lists all integrations for a specific project.
    ///
    /// Retrieves all integrations associated with the specified project,
    /// regardless of their status or activity state. Results are ordered by
    /// creation date with newest integrations first.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project whose integrations to retrieve
    ///
    /// # Returns
    ///
    /// A vector of all `ProjectIntegration` entries for the project, ordered by
    /// creation date (newest first), or a database error if the query fails.
    pub async fn list_project_integrations(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(project_id.eq(proj_id))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Finds all integrations matching a specific sync status.
    ///
    /// Retrieves integrations across all projects that have the specified
    /// sync status. This is useful for system-wide monitoring, identifying
    /// failed integrations, or finding integrations in specific operational states.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `integration_status` - Specific sync status to filter by
    ///
    /// # Returns
    ///
    /// A vector of `ProjectIntegration` entries with matching status, ordered by
    /// creation date (newest first), or a database error if the query fails.
    pub async fn find_integrations_by_status(
        conn: &mut AsyncPgConnection,
        integration_status: IntegrationStatus,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(sync_status.eq(Some(integration_status)))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Finds an integration by project and name combination.
    ///
    /// Searches for an integration within a specific project using its display name.
    /// Integration names should be unique within a project, making this a reliable
    /// way to locate specific integrations when you know both the project and name.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to search within
    /// * `name` - Exact integration name to search for
    ///
    /// # Returns
    ///
    /// The matching `ProjectIntegration` if found, `None` if not found,
    /// or a database error if the query fails.
    pub async fn find_integration_by_project_and_name(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
        name: &str,
    ) -> PgResult<Option<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integration = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(integration_name.eq(name))
            .select(ProjectIntegration::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(integration)
    }

    /// Enables an integration for active operation.
    ///
    /// Activates the specified integration, allowing it to participate in sync
    /// operations, receive webhooks, and perform automated tasks. The integration
    /// must be properly configured with credentials before activation.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration_id` - UUID of the integration to enable
    /// * `_updated_by` - UUID of the user performing the operation (for audit trails)
    ///
    /// # Returns
    ///
    /// The updated `ProjectIntegration` with active status set to true,
    /// or a database error if the operation fails.
    pub async fn enable_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            is_active: Some(true),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Disables an integration to stop all operations.
    ///
    /// Deactivates the specified integration, preventing it from performing sync
    /// operations, receiving webhooks, or executing automated tasks. The integration's
    /// configuration is preserved for potential reactivation.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration_id` - UUID of the integration to disable
    /// * `_updated_by` - UUID of the user performing the operation (for audit trails)
    ///
    /// # Returns
    ///
    /// The updated `ProjectIntegration` with active status set to false,
    /// or a database error if the operation fails.
    pub async fn disable_integration(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            is_active: Some(false),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Lists only active integrations for a specific project.
    ///
    /// Retrieves integrations that are currently enabled and operational within
    /// the specified project. This filters out disabled or deactivated integrations,
    /// showing only those that can participate in sync operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project whose active integrations to retrieve
    ///
    /// # Returns
    ///
    /// A vector of active `ProjectIntegration` entries for the project, ordered by
    /// creation date (newest first), or a database error if the query fails.
    pub async fn list_active_integrations(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Updates the sync status of an integration.
    ///
    /// Modifies the current sync status of an integration, typically used by
    /// sync processes to indicate operational state changes such as starting
    /// execution, completing successfully, or encountering failures.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration_id` - UUID of the integration to update
    /// * `new_status` - New sync status to set
    /// * `_updated_by` - UUID of the user or system performing the update
    ///
    /// # Returns
    ///
    /// The updated `ProjectIntegration` with new sync status,
    /// or a database error if the operation fails.
    pub async fn update_integration_status(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        new_status: IntegrationStatus,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            sync_status: Some(new_status),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Lists integrations that require administrator attention.
    ///
    /// Retrieves integrations that are in problematic states - either experiencing
    /// sync failures or are currently disabled. This is useful for monitoring
    /// dashboards and maintenance workflows to identify integrations that need
    /// intervention.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - Optional project ID to scope the search (None for system-wide)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectIntegration` entries that need attention, ordered by
    /// last update time (most recent first), or a database error if the query fails.
    pub async fn list_integrations_needing_attention(
        conn: &mut AsyncPgConnection,
        proj_id: Option<Uuid>,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let mut query = project_integrations.into_boxed();

        if let Some(pid) = proj_id {
            query = query.filter(project_id.eq(pid));
        }

        let integrations = query
            .filter(
                sync_status
                    .eq(Some(IntegrationStatus::Failure))
                    .or(is_active.eq(false)),
            )
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Gets comprehensive integration statistics for a project.
    ///
    /// Calculates key metrics about integrations within a project, including
    /// total count, active count, failed count, and pending count. These statistics
    /// provide a quick overview of integration health and status distribution.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to analyze
    ///
    /// # Returns
    ///
    /// A tuple containing (total_count, active_count, failed_count, pending_count),
    /// or a database error if the query fails.
    pub async fn get_integration_stats(
        conn: &mut AsyncPgConnection,
        proj_id: Uuid,
    ) -> PgResult<(i64, i64, i64, i64)> {
        use schema::project_integrations::dsl::*;

        // Count total integrations
        let total_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count active integrations
        let active_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(is_active.eq(true))
            .filter(sync_status.eq(Some(IntegrationStatus::Executing)))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count failed integrations
        let failed_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(sync_status.eq(Some(IntegrationStatus::Failure)))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Count pending integrations
        let pending_count: i64 = project_integrations
            .filter(project_id.eq(proj_id))
            .filter(sync_status.eq(Some(IntegrationStatus::Pending)))
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok((total_count, active_count, failed_count, pending_count))
    }

    /// Updates the authentication credentials for an integration.
    ///
    /// Replaces the stored authentication data with new credentials. This is
    /// typically used when API tokens expire, passwords change, or OAuth tokens
    /// need refreshing. The credentials should be encrypted before passing to
    /// this method for security.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration_id` - UUID of the integration to update
    /// * `auth_data` - New encrypted authentication credentials
    /// * `_updated_by` - UUID of the user performing the update
    ///
    /// # Returns
    ///
    /// The updated `ProjectIntegration` with new credentials,
    /// or a database error if the operation fails.
    ///
    /// # Security Note
    ///
    /// Credentials should be encrypted at the application layer before being
    /// stored in the database to ensure sensitive authentication data is protected.
    pub async fn update_integration_auth(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        auth_data: serde_json::Value,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            credentials: Some(auth_data),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Updates the configuration metadata for an integration.
    ///
    /// Replaces the stored metadata with new configuration data. This includes
    /// service-specific settings, sync preferences, webhook URLs, and other
    /// non-credential configuration that affects how the integration operates.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `integration_id` - UUID of the integration to update
    /// * `metadata` - New configuration metadata as JSON
    /// * `_updated_by` - UUID of the user performing the update
    ///
    /// # Returns
    ///
    /// The updated `ProjectIntegration` with new metadata,
    /// or a database error if the operation fails.
    pub async fn update_integration_metadata(
        conn: &mut AsyncPgConnection,
        integration_id: Uuid,
        metadata: serde_json::Value,
        _updated_by: Uuid,
    ) -> PgResult<ProjectIntegration> {
        let changes = UpdateProjectIntegration {
            metadata: Some(metadata),
            ..Default::default()
        };

        Self::update_integration(conn, integration_id, changes).await
    }

    /// Lists integrations created by a specific user with pagination.
    ///
    /// Retrieves all integrations that were originally created by the specified user,
    /// regardless of which projects they belong to. This is useful for user-specific
    /// views, permission management, and understanding user integration patterns.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `creator_id` - UUID of the user whose integrations to retrieve
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectIntegration` entries created by the user, ordered by
    /// creation date (newest first), or a database error if the query fails.
    pub async fn list_integrations_by_creator(
        conn: &mut AsyncPgConnection,
        creator_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let integrations = project_integrations
            .filter(created_by.eq(creator_id))
            .select(ProjectIntegration::as_select())
            .order(created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Finds integrations matching a name pattern across all projects.
    ///
    /// Searches for integrations with names that match the specified pattern using
    /// case-insensitive substring matching. This is useful for finding similarly
    /// named integrations across different projects or searching for integrations
    /// by partial names.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `name_pattern` - Pattern to search for in integration names (case-insensitive)
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `ProjectIntegration` entries with matching names, ordered by
    /// last update time (most recent first), or a database error if the query fails.
    pub async fn find_integrations_by_name_pattern(
        conn: &mut AsyncPgConnection,
        name_pattern: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let search_pattern = format!("%{}%", name_pattern);

        let integrations = project_integrations
            .filter(integration_name.ilike(&search_pattern))
            .select(ProjectIntegration::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Gets integrations that have been recently updated within a time window.
    ///
    /// Retrieves integrations that have been modified within the specified number
    /// of hours, optionally scoped to a specific project. This is useful for
    /// monitoring recent changes, tracking integration maintenance, and identifying
    /// active integrations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - Optional project ID to scope the search (None for system-wide)
    /// * `hours` - Number of hours back to search for updates
    ///
    /// # Returns
    ///
    /// A vector of up to 50 recently updated `ProjectIntegration` entries, ordered by
    /// A vector of `ProjectIntegration` entries updated within the time window,
    /// ordered by update time (most recent first), or a database error if the query fails.
    pub async fn get_recently_updated_integrations(
        conn: &mut AsyncPgConnection,
        proj_id: Option<Uuid>,
        hours: i64,
    ) -> PgResult<Vec<ProjectIntegration>> {
        use schema::project_integrations::dsl::*;

        let cutoff_time = OffsetDateTime::now_utc() - time::Duration::hours(hours);

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
            .load(conn)
            .await
            .map_err(PgError::from)?;

        Ok(integrations)
    }

    /// Checks if an integration name is unique within a project.
    ///
    /// Verifies that no other integration in the specified project has the same name,
    /// optionally excluding a specific integration ID. This is used to enforce name
    /// uniqueness constraints and prevent duplicate integration names within projects.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `proj_id` - UUID of the project to check within
    /// * `name` - Integration name to check for uniqueness
    /// * `exclude_id` - Optional integration ID to exclude from the check (for updates)
    ///
    /// # Returns
    ///
    /// `true` if the name is unique (available), `false` if already taken,
    /// or a database error if the query fails.
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
