//! Workspace invites table constraint violations.

use strum::EnumString;

/// Workspace invites table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceInviteConstraints {
    #[strum(serialize = "workspace_invites_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_invites_invitee_email_format")]
    InviteeEmailFormat,
}
