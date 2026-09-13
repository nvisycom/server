//! Invite service outputs.

use nvisy_postgres::model::{Account, Workspace, WorkspaceInvite, WorkspaceMember};

/// Outcome of creating a workspace invite.
///
/// The invitee must already be a platform account. An email that maps to no
/// account produces [`InviteOutcome::UnknownEmail`] and no invite row is created —
/// the caller reports success either way so the response cannot be used to probe
/// whether an account exists.
#[must_use]
pub enum InviteOutcome {
    /// The invite was created for an existing account.
    ///
    /// Boxed so this variant does not dominate the enum's size over the empty
    /// [`InviteOutcome::UnknownEmail`].
    Created(Box<CreatedInvite>),
    /// No account matches the email; nothing was created.
    UnknownEmail,
}

/// A created invitation with its recipient.
pub struct CreatedInvite {
    /// The persisted invitation.
    pub invite: WorkspaceInvite,
    /// The account the invitation was addressed to. Unused by this crate; exposed
    /// for callers that deliver email out-of-band (e.g. the hosted edition) and
    /// need the recipient's account details.
    #[allow(dead_code)]
    pub account: Account,
}

/// The membership minted by accepting an invite, with its account.
pub struct AcceptedInvite {
    /// The new membership row.
    pub member: WorkspaceMember,
    /// The account that joined.
    pub account: Account,
}

/// A previewed invite code: the workspace it grants access to and the invite.
pub struct InvitePreview {
    /// The workspace the code joins.
    pub workspace: Workspace,
    /// The invite backing the code.
    pub invite: WorkspaceInvite,
}
