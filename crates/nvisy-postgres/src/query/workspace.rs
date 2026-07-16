//! Workspace repository for managing workspace operations.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use pgtrgm::expression_methods::TrgmExpressionMethods;
use uuid::Uuid;

use crate::model::{NewWorkspace, UpdateWorkspace, Workspace};
use crate::types::OffsetPagination;
use crate::{PgConnection, PgError, PgResult, schema};

/// Maximum number of slug candidates tried before giving up when generating a
/// unique workspace slug (the preferred slug plus numeric suffixes).
const MAX_SLUG_ATTEMPTS: u32 = 100;

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

    /// Creates a workspace, resolving slug collisions with a numeric suffix.
    ///
    /// The `workspace.slug` is treated as the preferred slug. If it is already
    /// taken, the insert is retried with `-2`, `-3`, … suffixes until one
    /// succeeds. The retry is driven by the database's unique constraint, so it
    /// is race-safe: a concurrent insert of the same slug loses the race and is
    /// retried rather than silently colliding.
    fn create_workspace_with_unique_slug(
        &mut self,
        workspace: NewWorkspace,
    ) -> impl Future<Output = PgResult<Workspace>> + Send;

    /// Finds a workspace by ID, excluding soft-deleted workspaces.
    fn find_workspace_by_id(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Workspace>>> + Send;

    /// Finds a workspace by slug, excluding soft-deleted workspaces.
    fn find_workspace_by_slug(
        &mut self,
        slug: &str,
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

    async fn create_workspace_with_unique_slug(
        &mut self,
        workspace: NewWorkspace,
    ) -> PgResult<Workspace> {
        let preferred = workspace.slug.clone();

        // Attempt the preferred slug first, then `-2`, `-3`, … on collision.
        // Each attempt uses a fresh candidate cloned from the base record, so a
        // failed insert never consumes the data needed for the next try. The
        // loop is bounded; in practice the first suffix or two resolves any
        // realistic collision.
        for attempt in 0..MAX_SLUG_ATTEMPTS {
            let candidate = if attempt == 0 {
                workspace.clone()
            } else {
                let slug = preferred
                    .with_numeric_suffix(attempt + 1)
                    .ok_or_else(|| PgError::unexpected("workspace slug cannot be disambiguated"))?;
                NewWorkspace {
                    slug,
                    ..workspace.clone()
                }
            };

            match self.create_workspace(candidate).await {
                Ok(created) => return Ok(created),
                Err(error) if error.is_slug_conflict() => continue,
                Err(error) => return Err(error),
            }
        }

        Err(PgError::unexpected(
            "exhausted workspace slug generation attempts",
        ))
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

    async fn find_workspace_by_slug(&mut self, slug_value: &str) -> PgResult<Option<Workspace>> {
        use schema::workspaces::dsl::*;

        let workspace = workspaces
            .filter(slug.eq(slug_value))
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

        let workspace_list = workspaces
            .filter(deleted_at.is_null())
            .filter(display_name.trgm_similar_to(search_query))
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
