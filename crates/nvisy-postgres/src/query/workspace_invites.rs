//! Workspace invite repository for managing workspace invitation operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{NewWorkspaceInvite, UpdateWorkspaceInvite, WorkspaceInvite};
use crate::types::{
    CursorPage, CursorPagination, InviteFilter, InviteSortBy, InviteSortField, InviteStatus, keyset,
};
use crate::{Error, PgConnection, Result, schema};

/// Keyset for paginating workspace invites. Invites can be sorted by date or by
/// email, so the cursor carries whichever field the sort uses — the keyset
/// comparison must run on the same column it orders by, or paging drifts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "camelCase")]
pub enum InviteCursor {
    /// Sorted by creation time.
    Date {
        /// When the invite was created.
        created_at: Timestamp,
        /// Invite id (tiebreaker).
        id: Uuid,
    },
    /// Sorted by invitee email.
    Email {
        /// The invitee email (invites with a null email are excluded from this
        /// sort).
        email: String,
        /// Invite id (tiebreaker).
        id: Uuid,
    },
}

/// Repository for workspace invitation database operations.
///
/// Handles workspace invitations including creation, acceptance, rejection, and token
/// management with expiration tracking.
pub trait WorkspaceInviteRepository {
    /// Creates a new workspace invitation with secure token generation.
    fn create_workspace_invite(
        &mut self,
        invite: NewWorkspaceInvite,
    ) -> impl Future<Output = Result<WorkspaceInvite>> + Send;

    /// Finds a workspace invitation by its unique token string.
    fn find_workspace_invite_by_token(
        &mut self,
        token: &str,
    ) -> impl Future<Output = Result<Option<WorkspaceInvite>>> + Send;

    /// Finds an invitation by ID, scoped to its workspace.
    fn find_invite_in_workspace(
        &mut self,
        workspace_id: Uuid,
        invite_id: Uuid,
    ) -> impl Future<Output = Result<Option<WorkspaceInvite>>> + Send;

    /// Updates a workspace invitation with new values and status changes.
    fn update_workspace_invite(
        &mut self,
        invite_id: Uuid,
        changes: UpdateWorkspaceInvite,
    ) -> impl Future<Output = Result<WorkspaceInvite>> + Send;

    /// Accepts a workspace invitation and marks it as successfully processed.
    fn accept_workspace_invite(
        &mut self,
        invite_id: Uuid,
        _acceptor_id: Uuid,
    ) -> impl Future<Output = Result<WorkspaceInvite>> + Send;

    /// Rejects or declines a workspace invitation.
    fn reject_workspace_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = Result<WorkspaceInvite>> + Send;

    /// Cancels a workspace invitation before it can be used.
    fn cancel_workspace_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> impl Future<Output = Result<WorkspaceInvite>> + Send;

    /// Lists workspace invitations with cursor pagination.
    fn cursor_list_workspace_invites(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<InviteCursor>,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> impl Future<Output = Result<CursorPage<WorkspaceInvite>>> + Send;

    /// Finds a pending workspace invitation by workspace and email.
    fn find_pending_workspace_invite_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> impl Future<Output = Result<Option<WorkspaceInvite>>> + Send;
}

impl WorkspaceInviteRepository for PgConnection {
    async fn create_workspace_invite(
        &mut self,
        invite: NewWorkspaceInvite,
    ) -> Result<WorkspaceInvite> {
        use schema::workspace_invites;

        let invite = diesel::insert_into(workspace_invites::table)
            .values(&invite)
            .returning(WorkspaceInvite::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(invite)
    }

    async fn find_workspace_invite_by_token(
        &mut self,
        token: &str,
    ) -> Result<Option<WorkspaceInvite>> {
        use schema::workspace_invites::dsl::*;

        let invite = workspace_invites
            .filter(invite_token.eq(token))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(invite)
    }

    async fn find_invite_in_workspace(
        &mut self,
        workspace_id: Uuid,
        invite_id: Uuid,
    ) -> Result<Option<WorkspaceInvite>> {
        use schema::workspace_invites::{self, dsl};

        let invite = workspace_invites::table
            .filter(dsl::id.eq(invite_id))
            .filter(dsl::workspace_id.eq(workspace_id))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(invite)
    }

    async fn update_workspace_invite(
        &mut self,
        invite_id: Uuid,
        changes: UpdateWorkspaceInvite,
    ) -> Result<WorkspaceInvite> {
        use schema::workspace_invites::dsl::*;

        let invite = diesel::update(workspace_invites)
            .filter(id.eq(invite_id))
            .set(&changes)
            .returning(WorkspaceInvite::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)?;

        Ok(invite)
    }

    async fn accept_workspace_invite(
        &mut self,
        invite_id: Uuid,
        acceptor_id: Uuid,
    ) -> Result<WorkspaceInvite> {
        let changes = UpdateWorkspaceInvite {
            invite_status: Some(InviteStatus::Accepted),
            responded_at: Some(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
            updated_by: Some(acceptor_id),
        };

        self.update_workspace_invite(invite_id, changes).await
    }

    async fn reject_workspace_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> Result<WorkspaceInvite> {
        let changes = UpdateWorkspaceInvite {
            invite_status: Some(InviteStatus::Declined),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_workspace_invite(invite_id, changes).await
    }

    async fn cancel_workspace_invite(
        &mut self,
        invite_id: Uuid,
        updated_by_id: Uuid,
    ) -> Result<WorkspaceInvite> {
        let changes = UpdateWorkspaceInvite {
            invite_status: Some(InviteStatus::Canceled),
            updated_by: Some(updated_by_id),
            ..Default::default()
        };

        self.update_workspace_invite(invite_id, changes).await
    }

    async fn cursor_list_workspace_invites(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination<InviteCursor>,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> Result<CursorPage<WorkspaceInvite>> {
        use diesel::dsl::count_star;
        use schema::workspace_invites::{self, dsl};

        let sort_by_email = matches!(sort_by.field, InviteSortField::Email);
        // The invite sort order is the keyset direction, so the order-by and the
        // after-comparison always agree.
        let direction = sort_by.order;

        let base_filter = dsl::workspace_id
            .eq(workspace_id)
            .and(dsl::invite_status.ne(InviteStatus::Canceled));

        // The scoped builder (filters shared by the count and the page). When
        // sorting by email, null emails are excluded so the sort column is total.
        let scoped = || {
            let mut query = workspace_invites::table
                .filter(base_filter.clone())
                .into_boxed();
            if let Some(role) = filter.role {
                query = query.filter(dsl::invited_role.eq(role));
            }
            if sort_by_email {
                query = query.filter(dsl::invitee_email.is_not_null());
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(Error::from)?,
            )
        } else {
            None
        };

        // The keyset runs on whichever column the sort uses; the cursor carries the
        // matching value, so a stray Email cursor on a Date sort (or vice-versa)
        // simply starts a fresh page rather than drifting.
        let items = match sort_by.field {
            InviteSortField::Email => {
                let after = match pagination.after_key() {
                    Some(InviteCursor::Email { email, id }) => Some((email.clone(), *id)),
                    _ => None,
                };
                keyset!(scoped(), dsl::invitee_email, dsl::id, direction, after)
            }
            InviteSortField::Date => {
                let after = match pagination.after_key() {
                    Some(InviteCursor::Date { created_at, id }) => {
                        Some((jiff_diesel::Timestamp::from(*created_at), *id))
                    }
                    _ => None,
                };
                keyset!(scoped(), dsl::created_at, dsl::id, direction, after)
            }
        }
        .select(WorkspaceInvite::as_select())
        .limit(pagination.fetch_limit())
        .load(self)
        .await
        .map_err(Error::from)?;

        Ok(CursorPage::new(items, total, pagination.limit, move |i| {
            if sort_by_email {
                InviteCursor::Email {
                    // A null email cannot appear here — the sort filters them out.
                    email: i.invitee_email.clone().unwrap_or_default(),
                    id: i.id,
                }
            } else {
                InviteCursor::Date {
                    created_at: i.created_at.into(),
                    id: i.id,
                }
            }
        }))
    }

    async fn find_pending_workspace_invite_by_email(
        &mut self,
        workspace_id: Uuid,
        email: &str,
    ) -> Result<Option<WorkspaceInvite>> {
        use diesel::dsl::now;
        use schema::workspace_invites::{self, dsl};

        let invite = workspace_invites::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::invitee_email.eq(email))
            .filter(dsl::invite_status.eq(InviteStatus::Pending))
            .filter(dsl::expires_at.gt(now))
            .select(WorkspaceInvite::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)?;

        Ok(invite)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::PgConn;
    use crate::model::{NewWorkspaceInvite, WorkspaceInvite};
    use crate::query::{WorkspaceInviteRepository, WorkspaceRepository};
    use crate::test_util::TestDatabase;
    use crate::types::{Direction, InviteSortBy, InviteSortField, WorkspaceRole};

    /// Creates an invite addressed to `email` (the `New*` test constructor leaves
    /// `invitee_email` NULL, an open invite code, so set it on the struct).
    async fn invite_with_email(
        conn: &mut PgConn,
        workspace_id: Uuid,
        owner_id: Uuid,
        email: &str,
    ) -> anyhow::Result<WorkspaceInvite> {
        let mut new = NewWorkspaceInvite::test(workspace_id, owner_id);
        new.invitee_email = Some(email.to_owned());
        Ok(conn.create_workspace_invite(new).await?)
    }

    #[tokio::test]
    async fn create_defaults_and_lookups_are_workspace_scoped() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let invite = conn
            .create_workspace_invite(NewWorkspaceInvite::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        // Database defaults are applied.
        assert_eq!(invite.invite_status, InviteStatus::Pending);
        assert_eq!(invite.invited_role, WorkspaceRole::Reviewer);
        assert!(!invite.invite_token.trim().is_empty());
        assert!(jiff::Timestamp::from(invite.expires_at) > jiff::Timestamp::now());

        // Found by its token.
        let by_token = conn
            .find_workspace_invite_by_token(&invite.invite_token)
            .await?;
        assert_eq!(by_token.map(|i| i.id), Some(invite.id));

        // Found in its own workspace, not in another.
        assert!(
            conn.find_invite_in_workspace(seeded.workspace_id, invite.id)
                .await?
                .is_some()
        );
        let other_ws = conn
            .create_workspace(crate::model::NewWorkspace::test(seeded.account_id))
            .await?
            .id;
        assert!(
            conn.find_invite_in_workspace(other_ws, invite.id)
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn accept_reject_cancel_set_status_and_audit_fields() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Accept records a status and a response timestamp.
        let accepted = conn
            .create_workspace_invite(NewWorkspaceInvite::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let accepted = conn
            .accept_workspace_invite(accepted.id, seeded.account_id)
            .await?;
        assert_eq!(accepted.invite_status, InviteStatus::Accepted);
        assert!(accepted.responded_at.is_some());

        // Reject records the declining actor as `updated_by`.
        let rejected = conn
            .create_workspace_invite(NewWorkspaceInvite::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let rejected = conn
            .reject_workspace_invite(rejected.id, seeded.account_id)
            .await?;
        assert_eq!(rejected.invite_status, InviteStatus::Declined);
        assert_eq!(rejected.updated_by, seeded.account_id);

        // Cancel moves to Canceled.
        let canceled = conn
            .create_workspace_invite(NewWorkspaceInvite::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let canceled = conn
            .cancel_workspace_invite(canceled.id, seeded.account_id)
            .await?;
        assert_eq!(canceled.invite_status, InviteStatus::Canceled);
        Ok(())
    }

    #[tokio::test]
    async fn find_pending_by_email_matches_only_pending_unexpired_in_workspace()
    -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A pending invite with the target email matches.
        let pending = invite_with_email(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            "invitee@example.com",
        )
        .await?;
        let found = conn
            .find_pending_workspace_invite_by_email(seeded.workspace_id, "invitee@example.com")
            .await?;
        assert_eq!(found.map(|i| i.id), Some(pending.id));

        // An accepted invite with the same email does NOT match.
        let accepted = invite_with_email(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            "accepted@example.com",
        )
        .await?;
        let _ = conn
            .accept_workspace_invite(accepted.id, seeded.account_id)
            .await?;
        assert!(
            conn.find_pending_workspace_invite_by_email(
                seeded.workspace_id,
                "accepted@example.com"
            )
            .await?
            .is_none()
        );

        // The right email in the wrong workspace does not match.
        assert!(
            conn.find_pending_workspace_invite_by_email(Uuid::now_v7(), "invitee@example.com")
                .await?
                .is_none()
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_excludes_canceled_and_applies_role_filter() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // A pending reviewer invite, an editor invite, and a canceled one.
        let reviewer = conn
            .create_workspace_invite(NewWorkspaceInvite::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let mut editor = NewWorkspaceInvite::test(seeded.workspace_id, seeded.account_id);
        editor.invited_role = Some(WorkspaceRole::Editor);
        let editor = conn.create_workspace_invite(editor).await?;
        let canceled = conn
            .create_workspace_invite(NewWorkspaceInvite::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;
        let _ = conn
            .cancel_workspace_invite(canceled.id, seeded.account_id)
            .await?;

        let sort = InviteSortBy::new(InviteSortField::Date, Direction::Descending);

        // No role filter: both non-canceled invites, canceled excluded.
        let all = conn
            .cursor_list_workspace_invites(
                seeded.workspace_id,
                CursorPagination::new(50),
                sort,
                InviteFilter::default(),
            )
            .await?;
        let ids: Vec<_> = all.items.iter().map(|i| i.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&reviewer.id) && ids.contains(&editor.id));
        assert!(!ids.contains(&canceled.id));

        // Role filter narrows to the editor invite.
        let editors = conn
            .cursor_list_workspace_invites(
                seeded.workspace_id,
                CursorPagination::new(50),
                sort,
                InviteFilter {
                    role: Some(WorkspaceRole::Editor),
                },
            )
            .await?;
        assert_eq!(
            editors.items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![editor.id]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cursor_list_sorted_by_email_excludes_null_emails() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Two invites with emails, one open (null-email) code.
        let bravo = invite_with_email(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            "bravo@example.com",
        )
        .await?;
        let alpha = invite_with_email(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            "alpha@example.com",
        )
        .await?;
        let _open = conn
            .create_workspace_invite(NewWorkspaceInvite::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // Sorting by email ascending drops the null-email invite and orders the rest.
        let page = conn
            .cursor_list_workspace_invites(
                seeded.workspace_id,
                CursorPagination::new(50),
                InviteSortBy::new(InviteSortField::Email, Direction::Ascending),
                InviteFilter::default(),
            )
            .await?;
        assert_eq!(
            page.items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![alpha.id, bravo.id]
        );
        Ok(())
    }
}
