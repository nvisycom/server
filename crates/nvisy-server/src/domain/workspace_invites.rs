//! Workspace invite domain logic: send, list, cancel, reply, and invite codes.
//!
//! Holds the invitation rules — email/member/pending conflict checks, the
//! anti-enumeration no-op for an unknown email, invitee-email binding, and the
//! atomic accept that mints a membership — factored out of the handler so the
//! lifecycle and its events live in one place.

use nvisy_postgres::model::{NewWorkspaceMember, WorkspaceInvite};
use nvisy_postgres::query::{
    AccountRepository, InviteCursor, WorkspaceInviteRepository, WorkspaceMemberRepository,
    WorkspaceRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, InviteFilter, InviteSortBy};
use nvisy_postgres::{AsyncConnection, Error as PgError, PgClient, PgConn};
use uuid::Uuid;

use crate::domain::input::{CreateInviteInput, GenerateInviteCodeInput};
use crate::domain::output::{AcceptedInvite, CreatedInvite, InviteOutcome, InvitePreview};
use crate::extract::SecurityContext;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event;
use crate::service::event::EventEmitter;

/// Tracing target for invite domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::invite";

/// Sends, lists, cancels, and replies to workspace invites, and mints and
/// consumes invite codes.
///
/// Holds the Postgres client and acquires its own connection per call, so each
/// mutation is a self-contained transaction. Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspaceInviteService {
    postgres: PgClient,
}

impl WorkspaceInviteService {
    /// Creates a [`WorkspaceInviteService`] over the given connection pool.
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Creates a workspace invitation for an existing platform account.
    ///
    /// Rejects an email that already belongs to a member or has a pending invite.
    /// If the email resolves to an account, the invite is created and its
    /// `invite.created` event recorded in one transaction, returned as
    /// [`InviteOutcome::Created`]; otherwise [`InviteOutcome::UnknownEmail`] is
    /// returned without creating anything, so the response cannot reveal whether
    /// an account exists.
    ///
    /// A deployment that can deliver email out-of-band (e.g. the hosted edition)
    /// can call this, then send its own message on `Created` and handle
    /// `UnknownEmail` however it chooses.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        input: &CreateInviteInput,
    ) -> Result<InviteOutcome> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let actor_id = origin.account_id;

        // Normalize the email once so the member/account/pending lookups and the
        // stored invite all compare and persist the same canonical form.
        let invitee_email = input.normalized_email();

        if conn
            .find_workspace_member_by_email(workspace_id, &invitee_email)
            .await?
            .is_some()
        {
            return Err(ErrorKind::Conflict
                .with_message("User is already a member of this workspace")
                .with_resource("workspace_member"));
        }

        let Some(account) = conn.find_account_by_email(&invitee_email).await? else {
            return Ok(InviteOutcome::UnknownEmail);
        };

        if conn
            .find_pending_workspace_invite_by_email(workspace_id, &invitee_email)
            .await?
            .is_some()
        {
            return Err(ErrorKind::Conflict
                .with_message("A pending invitation already exists for this email")
                .with_resource("workspace_invite"));
        }

        let new_invite = input.to_model(workspace_id, actor_id);

        let invite = conn
            .transaction(async |conn| {
                let invite = conn.create_workspace_invite(new_invite).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::InviteCreated(event::InviteCreated {
                        invite_id: invite.id,
                        email: Some(invitee_email.clone()),
                    }),
                )
                .await?;
                Ok::<_, Error>(invite)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, invite_id = %invite.id, "Workspace invitation created");
        Ok(InviteOutcome::Created(Box::new(CreatedInvite {
            invite,
            account,
        })))
    }

    /// Lists a workspace's invitations, newest first.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<InviteCursor>,
        sort_by: InviteSortBy,
        filter: InviteFilter,
    ) -> Result<CursorPage<WorkspaceInvite>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_invites(workspace_id, pagination, sort_by, filter)
            .await?)
    }

    /// Cancels a pending invitation in a workspace, recording the event atomically.
    pub async fn cancel(&self, origin: event::EventOrigin<'_>, invite_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let workspace_id = origin.workspace_id;
        let actor_id = origin.account_id;

        let invite = find_invite(&mut conn, workspace_id, invite_id).await?;

        conn.transaction(async |conn| {
            conn.cancel_workspace_invite(invite_id, actor_id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::InviteCanceled(event::InviteCanceled {
                    invite_id: invite.id,
                    email: invite.invitee_email,
                }),
            )
            .await?;
            Ok::<_, Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Workspace invitation cancelled");
        Ok(())
    }

    /// Accepts an invitation by id, minting a membership. The invite must be usable
    /// and (if email-bound) addressed to the acting account.
    pub async fn accept(
        &self,
        origin: event::EventOrigin<'_>,
        invite_id: Uuid,
    ) -> Result<AcceptedInvite> {
        let mut conn = self.postgres.get_connection().await?;
        let invite = find_invite(&mut conn, origin.workspace_id, invite_id).await?;
        guard_usable(&invite)?;
        verify_invitee_matches(&mut conn, &invite, origin.account_id).await?;
        accept_invite_as_member(&mut conn, &invite, origin).await
    }

    /// Declines an invitation by id, recording the event atomically. The invite
    /// must be usable and (if email-bound) addressed to the acting account.
    pub async fn decline(&self, origin: event::EventOrigin<'_>, invite_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let invite = find_invite(&mut conn, origin.workspace_id, invite_id).await?;
        guard_usable(&invite)?;
        verify_invitee_matches(&mut conn, &invite, origin.account_id).await?;
        decline_invite(&mut conn, &invite, origin).await
    }

    /// Mints a shareable, single-use invite code for a workspace.
    pub async fn generate_code(
        &self,
        workspace_id: Uuid,
        actor_id: Uuid,
        input: GenerateInviteCodeInput,
    ) -> Result<WorkspaceInvite> {
        let mut conn = self.postgres.get_connection().await?;
        let invite = conn
            .create_workspace_invite(input.into_model(workspace_id, actor_id))
            .await?;
        tracing::info!(target: TRACING_TARGET, invite_id = %invite.id, "Workspace invite code generated");
        Ok(invite)
    }

    /// Previews an invite code: the workspace it grants access to. The code must be
    /// usable. Requires no authentication.
    pub async fn preview_code(&self, invite_code: &str) -> Result<InvitePreview> {
        let mut conn = self.postgres.get_connection().await?;
        let invite = find_code(&mut conn, invite_code).await?;
        if !invite.can_be_used() {
            return Err(ErrorKind::BadRequest
                .with_message("This invite code has expired or is no longer valid")
                .with_resource("invite_code"));
        }
        let Some(workspace) = conn.find_workspace_by_id(invite.workspace_id).await? else {
            return Err(ErrorKind::NotFound
                .with_resource("workspace")
                .with_message("Workspace not found"));
        };
        Ok(InvitePreview { workspace, invite })
    }

    /// Accepts an invite code, minting a membership. The code must be usable and
    /// (if email-bound) addressed to the acting account.
    pub async fn accept_code(
        &self,
        account_id: Uuid,
        security: &SecurityContext,
        invite_code: &str,
    ) -> Result<AcceptedInvite> {
        let mut conn = self.postgres.get_connection().await?;
        let invite = find_code(&mut conn, invite_code).await?;
        if !invite.can_be_used() {
            return Err(ErrorKind::BadRequest
                .with_message("This invite code has expired or is no longer valid")
                .with_resource("invite_code"));
        }
        verify_invitee_matches(&mut conn, &invite, account_id).await?;
        let origin = event::EventOrigin {
            workspace_id: invite.workspace_id,
            account_id,
            security,
        };
        accept_invite_as_member(&mut conn, &invite, origin).await
    }

    /// Declines an invite code, recording the event atomically. The code must be
    /// usable and (if email-bound) addressed to the acting account.
    pub async fn decline_code(
        &self,
        account_id: Uuid,
        security: &SecurityContext,
        invite_code: &str,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let invite = find_code(&mut conn, invite_code).await?;
        if !invite.can_be_used() {
            return Err(ErrorKind::BadRequest
                .with_message("This invite code has expired or is no longer valid")
                .with_resource("invite_code"));
        }
        verify_invitee_matches(&mut conn, &invite, account_id).await?;
        let origin = event::EventOrigin {
            workspace_id: invite.workspace_id,
            account_id,
            security,
        };
        decline_invite(&mut conn, &invite, origin).await
    }
}

/// Rejects an invite that has expired or been consumed.
fn guard_usable(invite: &WorkspaceInvite) -> Result<()> {
    if invite.can_be_used() {
        Ok(())
    } else {
        Err(ErrorKind::BadRequest
            .with_message("This invitation has expired or is no longer valid")
            .with_resource("workspace_invite"))
    }
}

/// Rejects an invite and records the decline event in one transaction, so the
/// event is never lost, nor recorded for a decline that rolled back.
async fn decline_invite(
    conn: &mut PgConn,
    invite: &WorkspaceInvite,
    origin: event::EventOrigin<'_>,
) -> Result<()> {
    let invite_id = invite.id;
    let email = invite.invitee_email.clone();
    conn.transaction(async |conn| {
        conn.reject_workspace_invite(invite_id, origin.account_id)
            .await?;
        conn.emit_event(
            origin,
            event::WorkspaceEvent::InviteDeclined(event::InviteDeclined { invite_id, email }),
        )
        .await?;
        Ok::<_, Error>(())
    })
    .await?;
    tracing::info!(target: TRACING_TARGET, "Invitation declined");
    Ok(())
}

/// Accepts an invite on behalf of an account and returns the new membership.
///
/// Rejects with a Conflict if the account is already a member, then, in a single
/// transaction, marks the invite accepted, adds the member, reads it back with its
/// account, and records the `InviteAccepted` and `MemberAdded` events — so the
/// membership and its events commit atomically. Shared by the invite-id and
/// invite-code accept paths.
async fn accept_invite_as_member(
    conn: &mut PgConn,
    invite: &WorkspaceInvite,
    origin: event::EventOrigin<'_>,
) -> Result<AcceptedInvite> {
    let account_id = origin.account_id;
    if conn
        .find_workspace_member(invite.workspace_id, account_id)
        .await?
        .is_some()
    {
        return Err(ErrorKind::Conflict
            .with_message("You are already a member of this workspace")
            .with_resource("workspace_member"));
    }

    let invite_id = invite.id;
    let workspace_id = invite.workspace_id;
    let invited_role = invite.invited_role;
    let email = invite.invitee_email.clone();

    let accepted = conn
        .transaction(async |conn| {
            // Atomic accept: `None` means the invite was consumed or expired
            // between the pre-check and here (a concurrent accept won the race).
            if conn
                .accept_workspace_invite(invite_id, account_id)
                .await?
                .is_none()
            {
                return Err(ErrorKind::Conflict
                    .with_message("This invitation is no longer valid")
                    .with_resource("workspace_invite"));
            }

            let new_member = NewWorkspaceMember::new(workspace_id, account_id, invited_role);
            conn.add_workspace_member(new_member).await?;

            let (member, account) = conn
                .find_workspace_member_with_account(workspace_id, account_id)
                .await?
                .ok_or_else(|| {
                    PgError::Unexpected("WorkspaceMember not found after insert".into())
                })?;

            conn.emit_event(
                origin,
                event::WorkspaceEvent::InviteAccepted(event::InviteAccepted { invite_id, email }),
            )
            .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::MemberAdded(event::MemberAdded {
                    member_id: account_id,
                    workspace_id,
                }),
            )
            .await?;

            Ok::<_, Error>(AcceptedInvite { member, account })
        })
        .await?;

    tracing::info!(target: TRACING_TARGET, "Invitation accepted");
    Ok(accepted)
}

/// Enforces that an email-bound invitation belongs to the authenticated account.
///
/// An email-bound invite may only be acted on (accepted *or* declined) by the
/// account that owns that email; otherwise any authenticated account could claim
/// or decline it. An open invite (no `invitee_email`) is exempt — it is claimable
/// by anyone who holds the code.
async fn verify_invitee_matches(
    conn: &mut PgConn,
    invite: &WorkspaceInvite,
    account_id: Uuid,
) -> Result<()> {
    let Some(ref invitee_email) = invite.invitee_email else {
        return Ok(());
    };
    let account = conn
        .find_account_by_id(account_id)
        .await?
        .ok_or_else(|| Error::not_found("account"))?;
    if account.email_address.eq_ignore_ascii_case(invitee_email) {
        Ok(())
    } else {
        Err(ErrorKind::Forbidden
            .with_message("This invitation was sent to a different email address")
            .with_resource("workspace_invite"))
    }
}

/// Finds an invite within a workspace, or a NotFound.
async fn find_invite(
    conn: &mut PgConn,
    workspace_id: Uuid,
    invite_id: Uuid,
) -> Result<WorkspaceInvite> {
    conn.find_invite_in_workspace(workspace_id, invite_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Invitation not found")
                .with_resource("workspace_invite")
        })
}

/// Finds an invite by its shareable code token, or a NotFound.
async fn find_code(conn: &mut PgConn, invite_code: &str) -> Result<WorkspaceInvite> {
    conn.find_workspace_invite_by_token(invite_code)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_resource("invite_code")
                .with_message("Invalid invite code")
        })
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use nvisy_postgres::model::NewAccount;
    use nvisy_postgres::test_util::TestDatabase;
    use nvisy_postgres::types::WorkspaceRole;

    use super::*;

    fn security() -> SecurityContext {
        SecurityContext::default()
    }

    /// A week from now, the expiry the invite endpoints stamp by default.
    fn in_seven_days() -> Option<Timestamp> {
        Timestamp::now()
            .checked_add(jiff::Span::new().hours(7 * 24))
            .ok()
    }

    fn invite_request(email: &str) -> CreateInviteInput {
        CreateInviteInput {
            invitee_email: email.to_owned(),
            invited_role: WorkspaceRole::Reviewer,
            expires_at: in_seven_days(),
        }
    }

    #[tokio::test]
    async fn create_then_accept_mints_a_membership() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceInviteService::new(db.client.clone());

        // The invitee must already be a platform account.
        let mut conn = db.client.get_connection().await?;
        let invitee = conn.create_account(NewAccount::test()).await?;
        drop(conn);

        let security = security();
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let outcome = service
            .create(origin, &invite_request(&invitee.email_address))
            .await?;
        let invite_id = match outcome {
            InviteOutcome::Created(created) => created.invite.id,
            InviteOutcome::UnknownEmail => panic!("expected an invite to be created"),
        };

        let accept_origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: invitee.id,
            security: &security,
        };
        let accepted = service.accept(accept_origin, invite_id).await?;
        assert_eq!(accepted.account.id, invitee.id);
        assert_eq!(accepted.member.member_role, WorkspaceRole::Reviewer);
        Ok(())
    }

    #[tokio::test]
    async fn create_is_a_no_op_for_an_unknown_email() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceInviteService::new(db.client.clone());
        let security = security();
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let outcome = service
            .create(origin, &invite_request("nobody@example.com"))
            .await?;
        assert!(matches!(outcome, InviteOutcome::UnknownEmail));
        Ok(())
    }

    #[tokio::test]
    async fn preview_code_returns_the_workspace() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspaceInviteService::new(db.client.clone());

        let invite = service
            .generate_code(
                seeded.workspace_id,
                seeded.account_id,
                GenerateInviteCodeInput {
                    invited_role: WorkspaceRole::Reviewer,
                    expires_at: in_seven_days(),
                },
            )
            .await?;

        let preview = service.preview_code(&invite.invite_token).await?;
        assert_eq!(preview.workspace.id, seeded.workspace_id);
        assert_eq!(preview.invite.id, invite.id);
        Ok(())
    }
}
