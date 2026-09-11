//! Workspace member repository for managing workspace membership.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    Account, NewWorkspaceMember, UpdateWorkspaceMember, Workspace, WorkspaceMember,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, Handle, MemberFilter, NotificationEvent,
    OffsetPagination, WorkspaceRole, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating an account's workspaces: newest membership first by
/// `created_at`, with the workspace id as the tiebreaker (a member row has a
/// composite key, so there is no single `id` column).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountWorkspaceCursor {
    /// When the membership was created.
    pub created_at: Timestamp,
    /// Workspace id (tiebreaker).
    pub workspace_id: uuid::Uuid,
}

/// Keyset for paginating a workspace's members: newest membership first by
/// `created_at`, with the account id as the tiebreaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMemberCursor {
    /// When the membership was created.
    pub created_at: Timestamp,
    /// Account id (tiebreaker).
    pub account_id: uuid::Uuid,
}

/// Repository for workspace member database operations.
///
/// Handles workspace membership management including CRUD operations, role-based
/// access control, and activity tracking.
pub trait WorkspaceMemberRepository {
    /// Adds a new member to a workspace.
    fn add_workspace_member(
        &mut self,
        member: NewWorkspaceMember,
    ) -> impl Future<Output = Result<WorkspaceMember>> + Send;

    /// Finds a workspace member by workspace and account IDs.
    fn find_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceMember>>> + Send;

    /// Resolves a set of usernames to the account ids of those that are members
    /// of the workspace, in one query. Non-members and unknown usernames are
    /// omitted; the result is deduplicated by account.
    fn find_member_ids_by_usernames(
        &mut self,
        workspace_id: Uuid,
        usernames: &[Handle],
    ) -> impl Future<Output = Result<Vec<Uuid>>> + Send;

    /// Updates a workspace member with partial changes.
    fn update_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateWorkspaceMember,
    ) -> impl Future<Output = Result<WorkspaceMember>> + Send;

    /// Permanently removes a member from a workspace.
    fn remove_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Lists user workspaces with full workspace details via JOIN.
    fn list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<(Workspace, WorkspaceMember)>>> + Send;

    /// Lists user workspaces with full workspace details using cursor
    /// pagination, each paired with the handle of the account that created the
    /// workspace.
    fn cursor_list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination<AccountWorkspaceCursor>,
    ) -> impl Future<Output = Result<CursorPage<(Workspace, WorkspaceMember, AccountRefRow)>>> + Send;

    /// Returns the account ids of members holding any of `roles` who accept
    /// `event` as an in-app notification.
    ///
    /// A member accepts the event when their `notification_events_app` is empty
    /// (the default, meaning "all") or contains it. One query resolves the whole
    /// recipient set for a broadcast.
    fn notification_recipients_by_roles(
        &mut self,
        workspace_id: Uuid,
        roles: &[WorkspaceRole],
        event: NotificationEvent,
    ) -> impl Future<Output = Result<Vec<Uuid>>> + Send;

    /// Lists members of a workspace with account details using cursor pagination.
    ///
    /// Returns members with their associated account information (email, display name).
    fn cursor_list_workspace_members_with_accounts(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<WorkspaceMemberCursor>,
        filter: MemberFilter,
    ) -> impl Future<Output = Result<CursorPage<(WorkspaceMember, Account)>>> + Send;

    /// Finds a workspace member with account details.
    fn find_workspace_member_with_account(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> impl Future<Output = Result<Option<(WorkspaceMember, Account)>>> + Send;

    /// Finds a workspace member by their email address.
    ///
    /// Performs a JOIN with accounts to match by email.
    fn find_workspace_member_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> impl Future<Output = Result<Option<(WorkspaceMember, Account)>>> + Send;

    /// Checks if two accounts share at least one common workspace.
    ///
    /// Returns true if both accounts are members of at least one common workspace.
    /// This is an optimized query that stops at the first match.
    fn accounts_share_workspace(
        &mut self,
        account_id_a: Uuid,
        account_id_b: Uuid,
    ) -> impl Future<Output = Result<bool>> + Send;
}

impl WorkspaceMemberRepository for PgConnection {
    async fn add_workspace_member(
        &mut self,
        member: NewWorkspaceMember,
    ) -> Result<WorkspaceMember> {
        use schema::workspace_members;

        let member = diesel::insert_into(workspace_members::table)
            .values(&member)
            .returning(WorkspaceMember::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(member)
    }

    async fn find_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> Result<Option<WorkspaceMember>> {
        use schema::workspace_members::{self, dsl};

        let member = workspace_members::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(member_account_id))
            .select(WorkspaceMember::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(member)
    }

    async fn find_member_ids_by_usernames(
        &mut self,
        workspace_id: Uuid,
        usernames: &[Handle],
    ) -> Result<Vec<Uuid>> {
        use schema::workspace_members::dsl as members;
        use schema::{accounts, workspace_members};

        if usernames.is_empty() {
            return Ok(Vec::new());
        }

        // Join members to their account and keep those whose username is in the
        // set — one round-trip instead of a lookup per handle.
        let ids: Vec<Uuid> = workspace_members::table
            .inner_join(accounts::table.on(members::account_id.eq(accounts::id)))
            .filter(members::workspace_id.eq(workspace_id))
            .filter(accounts::username.eq_any(usernames))
            .filter(accounts::deleted_at.is_null())
            .select(members::account_id)
            .load(self)
            .await
            .map_err(Error::from)?;

        Ok(ids)
    }

    async fn update_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
        changes: UpdateWorkspaceMember,
    ) -> Result<WorkspaceMember> {
        use schema::workspace_members::{self, dsl};

        let member = diesel::update(workspace_members::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(member_account_id))
            .set(&changes)
            .returning(WorkspaceMember::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(member)
    }

    async fn remove_workspace_member(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> Result<()> {
        use schema::workspace_members::{self, dsl};

        diesel::delete(workspace_members::table)
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::account_id.eq(member_account_id))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(())
    }

    async fn list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> Result<Vec<(Workspace, WorkspaceMember)>> {
        use schema::{workspace_members, workspaces};

        let results = workspace_members::table
            .inner_join(workspaces::table.on(workspaces::id.eq(workspace_members::workspace_id)))
            .filter(workspace_members::account_id.eq(account_id))
            .filter(workspaces::deleted_at.is_null())
            .select((Workspace::as_select(), WorkspaceMember::as_select()))
            .order(workspace_members::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .load::<(Workspace, WorkspaceMember)>(self)
            .await
            .map_err(Error::from)?;

        Ok(results)
    }

    async fn cursor_list_account_workspaces_with_details(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination<AccountWorkspaceCursor>,
    ) -> Result<CursorPage<(Workspace, WorkspaceMember, AccountRefRow)>> {
        use diesel::dsl::count_star;
        use schema::{accounts, workspace_members, workspaces};

        // Build base filter
        let base_filter = workspace_members::account_id
            .eq(account_id)
            .and(workspaces::deleted_at.is_null());

        // Get total count only if requested
        let total = if pagination.include_count {
            Some(
                workspace_members::table
                    .inner_join(
                        workspaces::table.on(workspaces::id.eq(workspace_members::workspace_id)),
                    )
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        let query = workspace_members::table
            .inner_join(workspaces::table.on(workspaces::id.eq(workspace_members::workspace_id)))
            .inner_join(accounts::table.on(accounts::id.eq(workspaces::created_by)))
            .filter(base_filter)
            .into_boxed();

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.workspace_id));
        let items = keyset!(
            query,
            workspace_members::created_at,
            workspace_members::workspace_id,
            pagination.direction,
            after
        )
        .limit(pagination.fetch_limit())
        .select((
            Workspace::as_select(),
            WorkspaceMember::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
        ))
        .load(self)
        .await
        .map_err(Error::from)?;

        Ok(CursorPage::new(
            items,
            total,
            pagination.limit,
            |(_, m, _): &(Workspace, WorkspaceMember, AccountRefRow)| AccountWorkspaceCursor {
                created_at: m.created_at.into(),
                workspace_id: m.workspace_id,
            },
        ))
    }

    async fn notification_recipients_by_roles(
        &mut self,
        workspace_id: Uuid,
        roles: &[WorkspaceRole],
        event: NotificationEvent,
    ) -> Result<Vec<Uuid>> {
        use schema::workspace_members::{self, dsl};

        // A member accepts the event when their in-app preference list is empty
        // (default = all) or contains it (`@>`). The array column is
        // Array<Nullable<NotificationEvent>>, so the needle matches that shape.
        let needle: Vec<Option<NotificationEvent>> = vec![Some(event)];
        let empty: Vec<Option<NotificationEvent>> = Vec::new();

        let account_ids = workspace_members::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::member_role.eq_any(roles.to_vec()))
            .filter(
                dsl::notification_events_app
                    .eq(empty)
                    .or(dsl::notification_events_app.contains(needle)),
            )
            .select(dsl::account_id)
            .load::<Uuid>(self)
            .await
            .map_err(Error::from)?;

        Ok(account_ids)
    }

    async fn cursor_list_workspace_members_with_accounts(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<WorkspaceMemberCursor>,
        filter: MemberFilter,
    ) -> Result<CursorPage<(WorkspaceMember, Account)>> {
        use diesel::dsl::count_star;
        use schema::{accounts, workspace_members};

        // Build base filter
        let base_filter = workspace_members::workspace_id
            .eq(workspace_id)
            .and(accounts::deleted_at.is_null());

        // Get total count only if requested
        let total = if pagination.include_count {
            let mut count_query = workspace_members::table
                .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
                .filter(base_filter)
                .into_boxed();

            if let Some(role) = filter.role {
                count_query = count_query.filter(workspace_members::member_role.eq(role));
            }

            Some(
                count_query
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        // Build query with optional role filter
        let mut query = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(base_filter)
            .into_boxed();

        if let Some(role) = filter.role {
            query = query.filter(workspace_members::member_role.eq(role));
        }

        let after = pagination
            .after_key()
            .map(|k| (jiff_diesel::Timestamp::from(k.created_at), k.account_id));
        let items = keyset!(
            query,
            workspace_members::created_at,
            workspace_members::account_id,
            pagination.direction,
            after
        )
        .limit(pagination.fetch_limit())
        .select((WorkspaceMember::as_select(), Account::as_select()))
        .load(self)
        .await
        .map_err(Error::from)?;

        Ok(CursorPage::new(items, total, pagination.limit, |(m, _)| {
            WorkspaceMemberCursor {
                created_at: m.created_at.into(),
                account_id: m.account_id,
            }
        }))
    }

    async fn find_workspace_member_with_account(
        &mut self,
        workspace_id: Uuid,
        member_account_id: Uuid,
    ) -> Result<Option<(WorkspaceMember, Account)>> {
        use schema::{accounts, workspace_members};

        let result = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(workspace_members::workspace_id.eq(workspace_id))
            .filter(workspace_members::account_id.eq(member_account_id))
            .filter(accounts::deleted_at.is_null())
            .select((WorkspaceMember::as_select(), Account::as_select()))
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(result)
    }

    async fn find_workspace_member_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> Result<Option<(WorkspaceMember, Account)>> {
        use schema::{accounts, workspace_members};

        let result = workspace_members::table
            .inner_join(accounts::table.on(accounts::id.eq(workspace_members::account_id)))
            .filter(workspace_members::workspace_id.eq(workspace_id))
            .filter(accounts::email_address.eq(email))
            .filter(accounts::deleted_at.is_null())
            .select((WorkspaceMember::as_select(), Account::as_select()))
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(result)
    }

    async fn accounts_share_workspace(
        &mut self,
        account_id_a: Uuid,
        account_id_b: Uuid,
    ) -> Result<bool> {
        use diesel::dsl::exists;
        use schema::workspace_members;

        // Self-check: an account always "shares" with itself
        if account_id_a == account_id_b {
            return Ok(true);
        }

        // Use EXISTS with a self-join to find any common workspace
        // This is optimized to stop at the first match
        let wm_a = workspace_members::table;
        let wm_b = diesel::alias!(workspace_members as wm_b);

        let shares = diesel::select(exists(
            wm_a.inner_join(
                wm_b.on(wm_b
                    .field(workspace_members::workspace_id)
                    .eq(workspace_members::workspace_id)),
            )
            .filter(workspace_members::account_id.eq(account_id_a))
            .filter(wm_b.field(workspace_members::account_id).eq(account_id_b)),
        ))
        .get_result::<bool>(self)
        .await
        .map_err(Error::from)?;

        Ok(shares)
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{NewAccount, NewWorkspace, NewWorkspaceMember, UpdateWorkspaceMember};
    use crate::query::{AccountRepository, WorkspaceMemberRepository, WorkspaceRepository};
    use crate::test_util::TestDatabase;
    use crate::types::{Handle, NotificationEvent, WorkspaceRole};

    #[tokio::test]
    async fn add_find_update_remove_round_trip() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let member = conn
            .add_workspace_member(NewWorkspaceMember::new(
                seeded.workspace_id,
                seeded.account_id,
                WorkspaceRole::Owner,
            ))
            .await?;
        assert_eq!(member.member_role, WorkspaceRole::Owner);

        let found = conn
            .find_workspace_member(seeded.workspace_id, seeded.account_id)
            .await?
            .expect("member should exist");
        assert_eq!(found.account_id, seeded.account_id);

        let updated = conn
            .update_workspace_member(
                seeded.workspace_id,
                seeded.account_id,
                UpdateWorkspaceMember {
                    member_role: Some(WorkspaceRole::Admin),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(updated.member_role, WorkspaceRole::Admin);

        conn.remove_workspace_member(seeded.workspace_id, seeded.account_id)
            .await?;
        assert!(
            conn.find_workspace_member(seeded.workspace_id, seeded.account_id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn notification_recipients_respect_role_and_event_prefs() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // An owner with default (empty) prefs accepts every event.
        let _ = conn
            .add_workspace_member(NewWorkspaceMember::new(
                seeded.workspace_id,
                seeded.account_id,
                WorkspaceRole::Owner,
            ))
            .await?;

        // A viewer who opted in to ONLY `member.joined`.
        let viewer_id = conn.create_account(NewAccount::test()).await?.id;
        let mut viewer =
            NewWorkspaceMember::new(seeded.workspace_id, viewer_id, WorkspaceRole::Reviewer);
        viewer.notification_events_app = vec![Some(NotificationEvent::MemberJoined)];
        let _ = conn.add_workspace_member(viewer).await?;

        // For `member.joined`, restricted to owners: only the owner matches.
        let owners_only = conn
            .notification_recipients_by_roles(
                seeded.workspace_id,
                &[WorkspaceRole::Owner],
                NotificationEvent::MemberJoined,
            )
            .await?;
        assert_eq!(owners_only, vec![seeded.account_id]);

        // For `member.joined` across owner+viewer: both accept it.
        let mut both = conn
            .notification_recipients_by_roles(
                seeded.workspace_id,
                &[WorkspaceRole::Owner, WorkspaceRole::Reviewer],
                NotificationEvent::MemberJoined,
            )
            .await?;
        both.sort();
        let mut expected = vec![seeded.account_id, viewer_id];
        expected.sort();
        assert_eq!(both, expected);

        // For an event the viewer did NOT opt into: only the all-events owner.
        let detection = conn
            .notification_recipients_by_roles(
                seeded.workspace_id,
                &[WorkspaceRole::Owner, WorkspaceRole::Reviewer],
                NotificationEvent::DetectionCompleted,
            )
            .await?;
        assert_eq!(detection, vec![seeded.account_id]);
        Ok(())
    }

    #[tokio::test]
    async fn accounts_share_workspace_detects_common_membership() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let member_id = conn.create_account(NewAccount::test()).await?.id;
        let stranger_id = conn.create_account(NewAccount::test()).await?.id;

        let _ = conn
            .add_workspace_member(NewWorkspaceMember::new(
                seeded.workspace_id,
                seeded.account_id,
                WorkspaceRole::Owner,
            ))
            .await?;
        let _ = conn
            .add_workspace_member(NewWorkspaceMember::new(
                seeded.workspace_id,
                member_id,
                WorkspaceRole::Editor,
            ))
            .await?;

        // Two members of the same workspace share it.
        assert!(
            conn.accounts_share_workspace(seeded.account_id, member_id)
                .await?
        );
        // The stranger is in no shared workspace.
        assert!(
            !conn
                .accounts_share_workspace(seeded.account_id, stranger_id)
                .await?
        );
        // An account always shares with itself, even with no memberships.
        assert!(
            conn.accounts_share_workspace(stranger_id, stranger_id)
                .await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn find_by_email_is_scoped_to_the_workspace() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let owner_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        // Two workspaces; the account is a member of only the first.
        let ws_a = conn
            .create_workspace(NewWorkspace::test(owner_id))
            .await?
            .id;
        let ws_b = conn
            .create_workspace(NewWorkspace::test(owner_id))
            .await?
            .id;

        let account_id = conn
            .create_account(NewAccount::new(Handle::test(), "member@example.com"))
            .await?
            .id;
        let _ = conn
            .add_workspace_member(NewWorkspaceMember::new(
                ws_a,
                account_id,
                WorkspaceRole::Editor,
            ))
            .await?;

        // Found in the workspace they belong to.
        let found = conn
            .find_workspace_member_by_email(ws_a, "member@example.com")
            .await?;
        assert_eq!(found.map(|(m, _)| m.account_id), Some(account_id));

        // Not found in the other workspace, even though the account exists.
        assert!(
            conn.find_workspace_member_by_email(ws_b, "member@example.com")
                .await?
                .is_none()
        );
        Ok(())
    }
}
