//! Workspace invites table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Workspace invites table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceInviteConstraints {
    #[strum(serialize = "workspace_invites_workspace_id_id_key")]
    WorkspaceIdIdUnique,
    #[strum(serialize = "workspace_invites_invitee_email_format")]
    InviteeEmailFormat,
}

impl WorkspaceInviteConstraints {
    /// Creates a new [`WorkspaceInviteConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }
}

impl From<WorkspaceInviteConstraints> for String {
    #[inline]
    fn from(val: WorkspaceInviteConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for WorkspaceInviteConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
