//! Workspace repository for managing workspace operations.

use std::future::Future;

use diesel::dsl::now;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspace, UpdateWorkspace, Workspace};
use crate::types::{AccountRefRow, WithAccountRef};
use crate::{Error, PgConnection, Result, schema};

/// Repository for workspace database operations.
///
/// Handles workspace lifecycle management: creation, lookup, updates, and
/// soft-deletion.
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
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use crate::model::{NewWorkspace, UpdateWorkspace};
    use crate::query::WorkspaceRepository;
    use crate::test_util::TestDatabase;

    #[tokio::test]
    async fn create_then_find_by_id_and_slug_round_trip() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let created = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await?;

        // By id.
        let by_id = conn.find_workspace_by_id(created.id).await?;
        assert_eq!(by_id.map(|w| w.id), Some(created.id));

        // By slug, carrying the creator's account reference.
        let by_slug = conn
            .find_workspace_by_slug(created.slug.as_str())
            .await?
            .context("workspace found by slug")?;
        assert_eq!(by_slug.item.id, created.id);
        assert_eq!(by_slug.item.display_name, created.display_name);
        Ok(())
    }

    #[tokio::test]
    async fn find_by_id_returns_none_for_missing_or_deleted() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        // A random id is not found.
        assert!(
            conn.find_workspace_by_id(uuid::Uuid::now_v7())
                .await?
                .is_none()
        );

        // A soft-deleted workspace is excluded.
        let ws = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await?;
        conn.delete_workspace(ws.id).await?;
        assert!(conn.find_workspace_by_id(ws.id).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn update_applies_changes_and_skips_deleted_rows() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let ws = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await?;

        let updated = conn
            .update_workspace(
                ws.id,
                UpdateWorkspace {
                    display_name: Some("Renamed".to_owned()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(updated.display_name, "Renamed");

        // Updating a soft-deleted workspace matches no live row and errors.
        conn.delete_workspace(ws.id).await?;
        let after_delete = conn
            .update_workspace(
                ws.id,
                UpdateWorkspace {
                    display_name: Some("Nope".to_owned()),
                    ..Default::default()
                },
            )
            .await;
        assert!(after_delete.is_err());
        Ok(())
    }
}
