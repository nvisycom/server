//! Workspace domain logic: create, read, update, delete, and list.
//!
//! Creating a workspace bootstraps its owner membership, so the workspace row,
//! the owner member, and the creation event commit together. Update and delete
//! each pair their write with an event in one transaction.

use nvisy_postgres::model::{Workspace, WorkspaceMember};
use nvisy_postgres::query::{
    AccountWorkspaceCursor, WorkspaceMemberRepository, WorkspaceRepository,
};
use nvisy_postgres::types::{AccountRefRow, CursorPage, CursorPagination};
use nvisy_postgres::{AsyncConnection, PgClient, model};
use uuid::Uuid;

use crate::domain::output::WorkspaceWithMembership;
use crate::response::{Error, Result};
use crate::service::event;
use crate::service::event::EventEmitter;

/// Tracing target for workspace domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::workspace";

/// Creates, reads, updates, deletes, and lists workspaces.
///
/// Holds the Postgres client and acquires its own connection per call, so each
/// mutation is a self-contained transaction. Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspaceService {
    postgres: PgClient,
}

impl WorkspaceService {
    /// Creates a [`WorkspaceService`] over the given connection pool.
    #[must_use]
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Creates a workspace with the acting account as its owner.
    ///
    /// The workspace, its owner membership, and the creation event commit in one
    /// transaction, so the event is never lost nor recorded for a workspace that
    /// rolled back.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        new_workspace: model::NewWorkspace,
    ) -> Result<WorkspaceWithMembership> {
        let mut conn = self.postgres.get_connection().await?;
        let creator_id = origin.account_id;

        let (workspace, membership) = conn
            .transaction(async |conn| {
                let workspace = conn.create_workspace(new_workspace).await?;
                let new_member = model::NewWorkspaceMember::new_owner(workspace.id, creator_id);
                let membership = conn.add_workspace_member(new_member).await?;
                conn.emit_event(
                    event::EventOrigin {
                        workspace_id: workspace.id,
                        account_id: creator_id,
                        security: origin.security,
                    },
                    event::WorkspaceEvent::WorkspaceCreated(event::WorkspaceCreated {
                        workspace_id: workspace.id,
                    }),
                )
                .await?;
                Ok::<_, Error>((workspace, membership))
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, workspace_handle = %workspace.handle, "Workspace created");
        Ok(WorkspaceWithMembership {
            workspace,
            membership,
        })
    }

    /// Lists the workspaces the account is a member of, newest first, each with
    /// the account's membership and the creator's public identity.
    pub async fn list(
        &self,
        account_id: Uuid,
        pagination: CursorPagination<AccountWorkspaceCursor>,
    ) -> Result<CursorPage<(Workspace, WorkspaceMember, AccountRefRow)>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_account_workspaces_with_details(account_id, pagination)
            .await?)
    }

    /// Updates a workspace's configuration, recording the event atomically, and
    /// returns the updated workspace.
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        updates: model::UpdateWorkspace,
    ) -> Result<Workspace> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;

        let workspace = conn
            .transaction(async |conn| {
                let workspace = conn.update_workspace(workspace_id, updates).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::WorkspaceUpdated(event::WorkspaceUpdated {
                        workspace_id: workspace.id,
                    }),
                )
                .await?;
                Ok::<_, Error>(workspace)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, "Workspace updated");
        Ok(workspace)
    }

    /// Soft-deletes a workspace, recording the event atomically.
    pub async fn delete(&self, origin: event::EventOrigin<'_>) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;

        conn.transaction(async |conn| {
            conn.delete_workspace(workspace_id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::WorkspaceDeleted(event::WorkspaceDeleted { workspace_id }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Workspace deleted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::query::{WorkspaceMemberRepository, WorkspaceRepository};
    use nvisy_postgres::test_util::TestDatabase;
    use uuid::Uuid;

    use super::*;
    use crate::extract::SecurityContext;

    fn security() -> SecurityContext {
        SecurityContext::default()
    }

    #[tokio::test]
    async fn create_bootstraps_the_owner_member() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let service = WorkspaceService::new(db.client.clone());
        let security = security();
        let origin = event::EventOrigin {
            workspace_id: Uuid::nil(),
            account_id,
            security: &security,
        };

        let created = service
            .create(origin, model::NewWorkspace::test(account_id))
            .await?;
        assert_eq!(created.membership.account_id, account_id);
        assert!(created.membership.member_role.is_owner());

        // The owner membership is persisted, not just returned.
        let mut conn = db.client.get_connection().await?;
        let found = conn
            .find_workspace_member(created.workspace.id, account_id)
            .await?
            .expect("owner membership present");
        assert!(found.member_role.is_owner());
        Ok(())
    }

    #[tokio::test]
    async fn update_changes_the_display_name() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceService::new(db.client.clone());
        let security = security();
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let updates = model::UpdateWorkspace {
            display_name: Some("Renamed".to_owned()),
            ..Default::default()
        };
        let updated_workspace = service.update(origin, updates).await?;
        assert_eq!(updated_workspace.display_name, "Renamed");
        Ok(())
    }

    #[tokio::test]
    async fn delete_soft_deletes_the_workspace() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceService::new(db.client.clone());
        let security = security();
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        service.delete(origin).await?;

        let mut conn = db.client.get_connection().await?;
        assert!(
            conn.find_workspace_by_id(seeded.workspace_id)
                .await?
                .is_none(),
            "a soft-deleted workspace is not found"
        );
        Ok(())
    }
}
