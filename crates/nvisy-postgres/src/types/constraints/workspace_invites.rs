//! Workspace invites table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Workspace invites table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum WorkspaceInviteConstraints {
    // Invite validation constraints
    #[strum(serialize = "workspace_invites_invite_message_length_max")]
    InviteMessageLengthMax,
    #[strum(serialize = "workspace_invites_invite_token_not_empty")]
    InviteTokenNotEmpty,

    // Invite chronological constraints
    #[strum(serialize = "workspace_invites_expires_after_created")]
    ExpiresAfterCreated,
    #[strum(serialize = "workspace_invites_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "workspace_invites_responded_after_created")]
    RespondedAfterCreated,
}

impl WorkspaceInviteConstraints {
    /// Creates a new [`WorkspaceInviteConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            WorkspaceInviteConstraints::InviteMessageLengthMax
            | WorkspaceInviteConstraints::InviteTokenNotEmpty => ConstraintCategory::Validation,

            WorkspaceInviteConstraints::ExpiresAfterCreated
            | WorkspaceInviteConstraints::UpdatedAfterCreated
            | WorkspaceInviteConstraints::RespondedAfterCreated => ConstraintCategory::Chronological,
        }
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
