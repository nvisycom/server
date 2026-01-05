//! Workspace repository for managing workspace operations.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspace, UpdateWorkspace, Workspace};
use crate::types::OffsetPagination;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for workspace database operations.
///
/// Handles workspace lifecycle management including creation, updates,
/// and search functionality.
pub trait WorkspaceRepository {
    /// Creates a new workspace.
    ///
    /// Inserts a new workspace record with the provided configuration.
    fn create_workspace(
        &mut self,
        workspace: NewWorkspace,
    ) -> impl Future<Output = PgResult<Workspace>> + Send;

    /// Finds a workspace by ID, excluding soft-deleted workspaces.
    fn find_workspace_by_id(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Workspace>>> + Send;

    /// Updates a workspace with partial changes.
    fn update_workspace(
        &mut self,
        workspace_id: Uuid,
        changes: UpdateWorkspace,
    ) -> impl Future<Output = PgResult<Workspace>> + Send;

    /// Soft deletes a workspace by setting the deletion timestamp.
    fn delete_workspace(&mut self, workspace_id: Uuid)
    -> impl Future<Output = PgResult<()>> + Send;

    /// Lists workspaces.
    ///
    /// Returns workspaces ordered by update time with most recent first.
    fn list_workspaces(
        &mut self,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<Workspace>>> + Send;

    /// Searches workspaces by name or description.
    ///
    /// Performs case-insensitive search across workspace names and descriptions.
    fn search_workspaces(
        &mut self,
        search_query: &str,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<Workspace>>> + Send;

    /// Finds workspaces with overlapping tags.
    fn find_workspaces_by_tags(
        &mut self,
        search_tags: &[String],
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<Workspace>>> + Send;
}

impl WorkspaceRepository for PgConnection {
    async fn create_workspace(&mut self, workspace: NewWorkspace) -> PgResult<Workspace> {
        use schema::workspaces;

        let workspace = diesel::insert_into(workspaces::table)
            .values(&workspace)
            .returning(Workspace::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(workspace)
    }

    async fn find_workspace_by_id(&mut self, workspace_id: Uuid) -> PgResult<Option<Workspace>> {
        use schema::workspaces::dsl::*;

        let workspace = workspaces
            .filter(id.eq(workspace_id))
            .filter(deleted_at.is_null())
            .select(Workspace::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(workspace)
    }

    async fn update_workspace(
        &mut self,
        workspace_id: Uuid,
        changes: UpdateWorkspace,
    ) -> PgResult<Workspace> {
        use schema::workspaces::dsl::*;

        let workspace = diesel::update(workspaces)
            .filter(id.eq(workspace_id))
            .filter(deleted_at.is_null())
            .set(&changes)
            .returning(Workspace::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(workspace)
    }

    async fn delete_workspace(&mut self, workspace_id: Uuid) -> PgResult<()> {
        use schema::workspaces::dsl::*;

        diesel::update(workspaces)
            .filter(id.eq(workspace_id))
            .filter(deleted_at.is_null())
            .set(deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    async fn list_workspaces(&mut self, pagination: OffsetPagination) -> PgResult<Vec<Workspace>> {
        use schema::workspaces::dsl::*;

        let workspace_list = workspaces
            .filter(deleted_at.is_null())
            .select(Workspace::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(workspace_list)
    }

    async fn search_workspaces(
        &mut self,
        search_query: &str,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<Workspace>> {
        use schema::workspaces::dsl::*;

        let search_pattern = format!("%{}%", search_query);

        let workspace_list = workspaces
            .filter(deleted_at.is_null())
            .filter(diesel::BoolExpressionMethods::or(
                display_name.ilike(&search_pattern),
                description.ilike(&search_pattern),
            ))
            .select(Workspace::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(workspace_list)
    }

    async fn find_workspaces_by_tags(
        &mut self,
        search_tags: &[String],
        pagination: OffsetPagination,
    ) -> PgResult<Vec<Workspace>> {
        use schema::workspaces::dsl::*;

        let workspace_list = workspaces
            .filter(tags.overlaps_with(search_tags))
            .filter(deleted_at.is_null())
            .select(Workspace::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(PgError::from)?;

        Ok(workspace_list)
    }
}
