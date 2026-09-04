//! Workspace repository for managing workspace operations.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use uuid::Uuid;

use crate::model::{NewWorkspace, UpdateWorkspace, Workspace};
use crate::query::search::ilike_contains;
use crate::types::{AccountRefRow, OffsetPagination, WithAccountRef};
use crate::{Error, PgConnection, Result, schema};

/// Repository for workspace database operations.
///
/// Handles workspace lifecycle management including creation, updates,
/// and search functionality.
pub trait WorkspaceRepository {
    /// Creates a new workspace.
    ///
    /// Inserts a new workspace record with the provided configuration. A slug or
    /// display-name collision surfaces as a unique-constraint error for the
    /// caller to turn into a client error.
    fn create_workspace(
        &mut self,
        workspace: NewWorkspace,
    ) -> impl Future<Output = Result<Workspace>> + Send;

    /// Finds a workspace by ID, excluding soft-deleted workspaces.
    fn find_workspace_by_id(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = Result<Option<Workspace>>> + Send;

    /// Finds a workspace by slug, with the handle and avatar of the account that
    /// created it, excluding soft-deleted workspaces.
    fn find_workspace_by_slug(
        &mut self,
        slug: &str,
    ) -> impl Future<Output = Result<Option<WithAccountRef<Workspace>>>> + Send;

    /// Updates a workspace with partial changes.
    fn update_workspace(
        &mut self,
        workspace_id: Uuid,
        changes: UpdateWorkspace,
    ) -> impl Future<Output = Result<Workspace>> + Send;

    /// Soft deletes a workspace by setting the deletion timestamp.
    fn delete_workspace(&mut self, workspace_id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Lists workspaces.
    ///
    /// Returns workspaces ordered by update time with most recent first.
    fn list_workspaces(
        &mut self,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<Workspace>>> + Send;

    /// Searches workspaces by name or description.
    ///
    /// Performs case-insensitive search across workspace names and descriptions.
    fn search_workspaces(
        &mut self,
        search_query: &str,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<Workspace>>> + Send;
}

impl WorkspaceRepository for PgConnection {
    async fn create_workspace(&mut self, workspace: NewWorkspace) -> Result<Workspace> {
        use schema::workspaces;

        let workspace = diesel::insert_into(workspaces::table)
            .values(&workspace)
            .returning(Workspace::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(workspace)
    }

    async fn find_workspace_by_id(&mut self, workspace_id: Uuid) -> Result<Option<Workspace>> {
        use schema::workspaces::dsl::*;

        let workspace = workspaces
            .filter(id.eq(workspace_id))
            .filter(deleted_at.is_null())
            .select(Workspace::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(workspace)
    }

    async fn find_workspace_by_slug(
        &mut self,
        slug_value: &str,
    ) -> Result<Option<WithAccountRef<Workspace>>> {
        use schema::workspaces::dsl;
        use schema::{accounts, workspaces};

        let row = workspaces::table
            .inner_join(accounts::table)
            .filter(dsl::slug.eq(slug_value))
            .filter(dsl::deleted_at.is_null())
            .select((
                Workspace::as_select(),
                (
                    accounts::username,
                    accounts::display_name,
                    accounts::avatar_url,
                ),
            ))
            .first::<(Workspace, AccountRefRow)>(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(row.map(|(item, account)| WithAccountRef { item, account }))
    }

    async fn update_workspace(
        &mut self,
        workspace_id: Uuid,
        changes: UpdateWorkspace,
    ) -> Result<Workspace> {
        use schema::workspaces::dsl::*;

        let workspace = diesel::update(workspaces)
            .filter(id.eq(workspace_id))
            .filter(deleted_at.is_null())
            .set(&changes)
            .returning(Workspace::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(workspace)
    }

    async fn delete_workspace(&mut self, workspace_id: Uuid) -> Result<()> {
        use schema::workspaces::dsl::*;

        diesel::update(workspaces)
            .filter(id.eq(workspace_id))
            .filter(deleted_at.is_null())
            .set(deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn list_workspaces(&mut self, pagination: OffsetPagination) -> Result<Vec<Workspace>> {
        use schema::workspaces::dsl::*;

        let workspace_list = workspaces
            .filter(deleted_at.is_null())
            .select(Workspace::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(workspace_list)
    }

    async fn search_workspaces(
        &mut self,
        search_query: &str,
        pagination: OffsetPagination,
    ) -> Result<Vec<Workspace>> {
        use schema::workspaces::dsl::*;

        let workspace_list = workspaces
            .filter(deleted_at.is_null())
            .filter(
                display_name
                    .ilike(ilike_contains(search_query))
                    .or(display_name.trgm_similar_to(search_query)),
            )
            .select(Workspace::as_select())
            .order(updated_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(workspace_list)
    }
}
