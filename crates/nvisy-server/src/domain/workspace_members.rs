//! Workspace member domain logic: list, read, update role, remove, and leave.
//!
//! Holds the membership rules — the self-action and owner guards, and the
//! last-owner check that a leave must pass — factored out of the handler so the
//! transitions and their events live in one place.

use nvisy_postgres::model::{Account, WorkspaceMember};
use nvisy_postgres::query::{WorkspaceMemberCursor, WorkspaceMemberRepository};
use nvisy_postgres::types::{CursorPage, CursorPagination, MemberFilter, MemberSortBy};
use nvisy_postgres::{AsyncConnection, PgClient, model};
use uuid::Uuid;

use crate::response::{Error, ErrorKind, Result};
use crate::service::event;
use crate::service::event::EventEmitter;

/// Tracing target for member domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::member";

/// Lists, reads, updates, and removes workspace members.
///
/// Holds the Postgres client and acquires its own connection per call, so each
/// mutation is a self-contained transaction. Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspaceMemberService {
    postgres: PgClient,
}

impl WorkspaceMemberService {
    /// Creates a [`WorkspaceMemberService`] over the given connection pool.
    #[must_use]
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Lists a workspace's members with their accounts, newest first.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<WorkspaceMemberCursor>,
        sort_by: MemberSortBy,
        filter: MemberFilter,
    ) -> Result<CursorPage<(WorkspaceMember, Account)>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_members_with_accounts(workspace_id, pagination, sort_by, filter)
            .await?)
    }

    /// Finds a member by account id with their account, or a `NotFound`.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> Result<(WorkspaceMember, Account)> {
        let mut conn = self.postgres.get_connection().await?;
        conn.find_workspace_member_with_account(workspace_id, member_account_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message("Workspace member not found"))
    }

    /// Removes a member from a workspace, recording the event atomically.
    ///
    /// The actor cannot remove themselves (they leave instead), and an owner
    /// cannot be removed (an owner can only leave).
    pub async fn remove(
        &self,
        origin: event::EventOrigin<'_>,
        member_account_id: Uuid,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let actor_id = origin.account_id;

        if actor_id == member_account_id {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot remove yourself. Use the leave workspace endpoint instead"));
        }

        let Some(member_to_remove) = conn
            .find_workspace_member(workspace_id, member_account_id)
            .await?
        else {
            return Err(ErrorKind::NotFound.into_error());
        };

        if member_to_remove.member_role.is_owner() {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot remove an owner")
                .with_context("Owners can only leave the workspace themselves"));
        }

        conn.transaction(async |conn| {
            conn.remove_workspace_member(workspace_id, member_account_id)
                .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::MemberDeleted(event::MemberDeleted {
                    member_id: member_account_id,
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Workspace member removed");
        Ok(())
    }

    /// Updates a member's role, recording the event atomically, and returns the
    /// updated member with their account.
    ///
    /// The actor cannot change their own role, and an owner cannot be demoted (an
    /// owner can only leave).
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        member_account_id: Uuid,
        updates: model::UpdateWorkspaceMember,
    ) -> Result<(WorkspaceMember, Account)> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let actor_id = origin.account_id;

        if actor_id == member_account_id {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot update your own role")
                .with_context("Ask another owner to update your role"));
        }

        let Some(current_member) = conn
            .find_workspace_member(workspace_id, member_account_id)
            .await?
        else {
            return Err(ErrorKind::NotFound.into_error());
        };

        // An owner cannot be demoted (only they can leave): reject a role change
        // that would move an owner to any non-owner role.
        let demotes_owner = current_member.member_role.is_owner()
            && updates.member_role.is_some_and(|role| !role.is_owner());
        if demotes_owner {
            return Err(ErrorKind::BadRequest
                .with_message("Cannot demote an owner")
                .with_context("Owners can only leave the workspace themselves"));
        }

        conn.transaction(async |conn| {
            conn.update_workspace_member(workspace_id, member_account_id, updates)
                .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::MemberUpdated(event::MemberUpdated {
                    member_id: member_account_id,
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        conn.find_workspace_member_with_account(workspace_id, member_account_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.into_error())
    }

    /// Returns the acting account's membership in the workspace, or a `NotFound`.
    ///
    /// The membership carries the account's notification preferences, so the
    /// handler reads its settings from the returned row.
    pub async fn notification_settings(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> Result<WorkspaceMember> {
        let mut conn = self.postgres.get_connection().await?;
        conn.find_workspace_member(workspace_id, account_id)
            .await?
            .ok_or_else(|| ErrorKind::NotFound.with_message("Workspace membership not found"))
    }

    /// Updates the acting account's notification preferences on its membership,
    /// returning the updated member. Fails with `NotFound` when the account is not a
    /// member of the workspace.
    pub async fn update_notification_settings(
        &self,
        workspace_id: Uuid,
        account_id: Uuid,
        updates: model::UpdateWorkspaceMember,
    ) -> Result<WorkspaceMember> {
        let mut conn = self.postgres.get_connection().await?;
        if conn
            .find_workspace_member(workspace_id, account_id)
            .await?
            .is_none()
        {
            return Err(ErrorKind::NotFound.with_message("Workspace membership not found"));
        }

        Ok(conn
            .update_workspace_member(workspace_id, account_id, updates)
            .await?)
    }

    /// Removes the acting member from a workspace (a voluntary leave), recording
    /// the departure atomically.
    ///
    /// A self-initiated leave is the same domain fact as an admin removal, so it
    /// records `MemberDeleted` with the leaving account as both actor and subject.
    /// The sole owner cannot leave and orphan the workspace: they must transfer
    /// ownership first.
    pub async fn leave(&self, origin: event::EventOrigin<'_>) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let account_id = origin.account_id;

        if conn
            .find_workspace_member(workspace_id, account_id)
            .await?
            .is_none()
        {
            return Err(ErrorKind::NotFound.with_message("You are not a member of this workspace"));
        }

        conn.transaction(async |conn| {
            // Read the owner set under a row lock inside this transaction so the
            // role is current and two owners leaving at once cannot both pass.
            let owner_ids = conn.lock_owner_ids(workspace_id).await?;
            if owner_ids.contains(&account_id) && owner_ids.len() <= 1 {
                return Err(ErrorKind::Conflict
                    .with_message("You are the only owner; transfer ownership before leaving"));
            }

            conn.remove_workspace_member(workspace_id, account_id)
                .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::MemberDeleted(event::MemberDeleted {
                    member_id: account_id,
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Workspace member left workspace");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::model::{NewAccount, NewWorkspaceMember};
    use nvisy_postgres::query::AccountRepository;
    use nvisy_postgres::test_util::TestDatabase;
    use nvisy_postgres::types::WorkspaceRole;

    use super::*;
    use crate::extract::SecurityContext;

    fn security() -> SecurityContext {
        SecurityContext::default()
    }

    /// Seeds a second account and adds it to the workspace with `role`, returning
    /// its account id.
    async fn add_member(db: &TestDatabase, workspace_id: Uuid, role: WorkspaceRole) -> Uuid {
        let mut conn = db.client.get_connection().await.expect("connection");
        let account = conn
            .create_account(NewAccount::test())
            .await
            .expect("account");
        conn.add_workspace_member(NewWorkspaceMember::new(workspace_id, account.id, role))
            .await
            .expect("member");
        account.id
    }

    #[tokio::test]
    async fn update_changes_a_member_role() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceMemberService::new(db.client.clone());
        let member = add_member(&db, seeded.workspace_id, WorkspaceRole::Reviewer).await;
        let security = security();
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let updates = model::UpdateWorkspaceMember {
            member_role: Some(WorkspaceRole::Editor),
            ..Default::default()
        };
        let (updated_member, _account) = service.update(origin, member, updates).await?;
        assert_eq!(updated_member.member_role, WorkspaceRole::Editor);
        Ok(())
    }

    #[tokio::test]
    async fn update_rejects_changing_your_own_role() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceMemberService::new(db.client.clone());
        // The acting account is a member of its own workspace.
        let mut conn = db.client.get_connection().await?;
        conn.add_workspace_member(NewWorkspaceMember::new(
            seeded.workspace_id,
            seeded.account_id,
            WorkspaceRole::Owner,
        ))
        .await?;
        drop(conn);
        let security = security();
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let updates = model::UpdateWorkspaceMember {
            member_role: Some(WorkspaceRole::Editor),
            ..Default::default()
        };
        let err = service
            .update(origin, seeded.account_id, updates)
            .await
            .expect_err("cannot update your own role");
        assert_eq!(err.kind(), ErrorKind::BadRequest);
        Ok(())
    }

    #[tokio::test]
    async fn update_notification_settings_changes_email_preference() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceMemberService::new(db.client.clone());
        // The acting account is a member of its own workspace.
        let mut conn = db.client.get_connection().await?;
        conn.add_workspace_member(NewWorkspaceMember::new(
            seeded.workspace_id,
            seeded.account_id,
            WorkspaceRole::Owner,
        ))
        .await?;
        drop(conn);

        let updates = model::UpdateWorkspaceMember {
            notify_via_email: Some(false),
            ..Default::default()
        };
        let member = service
            .update_notification_settings(seeded.workspace_id, seeded.account_id, updates)
            .await?;
        assert!(!member.notify_via_email);

        let read = service
            .notification_settings(seeded.workspace_id, seeded.account_id)
            .await?;
        assert!(!read.notify_via_email);
        Ok(())
    }

    #[tokio::test]
    async fn notification_settings_rejects_a_non_member() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceMemberService::new(db.client.clone());

        let err = service
            .notification_settings(seeded.workspace_id, Uuid::new_v4())
            .await
            .expect_err("a non-member has no settings");
        assert_eq!(err.kind(), ErrorKind::NotFound);
        Ok(())
    }

    #[tokio::test]
    async fn remove_rejects_an_owner() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceMemberService::new(db.client.clone());
        let owner = add_member(&db, seeded.workspace_id, WorkspaceRole::Owner).await;
        let security = security();
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let err = service
            .remove(origin, owner)
            .await
            .expect_err("cannot remove an owner");
        assert_eq!(err.kind(), ErrorKind::BadRequest);
        Ok(())
    }
}
