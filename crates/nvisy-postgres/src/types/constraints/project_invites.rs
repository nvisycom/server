//! Project invites table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Project invites table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum ProjectInviteConstraints {
    // Invite validation constraints
    #[strum(serialize = "project_invites_invite_message_length_max")]
    InviteMessageLengthMax,
    #[strum(serialize = "project_invites_invite_token_not_empty")]
    InviteTokenNotEmpty,

    // Invite chronological constraints
    #[strum(serialize = "project_invites_expires_after_created")]
    ExpiresAfterCreated,
    #[strum(serialize = "project_invites_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "project_invites_responded_after_created")]
    RespondedAfterCreated,
}

impl ProjectInviteConstraints {
    /// Creates a new [`ProjectInviteConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectInviteConstraints::InviteMessageLengthMax
            | ProjectInviteConstraints::InviteTokenNotEmpty => ConstraintCategory::Validation,

            ProjectInviteConstraints::ExpiresAfterCreated
            | ProjectInviteConstraints::UpdatedAfterCreated
            | ProjectInviteConstraints::RespondedAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<ProjectInviteConstraints> for String {
    #[inline]
    fn from(val: ProjectInviteConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for ProjectInviteConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
