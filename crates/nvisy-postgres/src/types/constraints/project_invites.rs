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
    #[strum(serialize = "project_invites_email_valid")]
    EmailValid,
    #[strum(serialize = "project_invites_invite_message_length_max")]
    InviteMessageLengthMax,
    #[strum(serialize = "project_invites_invite_token_not_empty")]
    InviteTokenNotEmpty,
    #[strum(serialize = "project_invites_status_reason_length_max")]
    StatusReasonLengthMax,
    #[strum(serialize = "project_invites_max_uses_min")]
    MaxUsesMin,
    #[strum(serialize = "project_invites_max_uses_max")]
    MaxUsesMax,
    #[strum(serialize = "project_invites_use_count_min")]
    UseCountMin,
    #[strum(serialize = "project_invites_use_count_max")]
    UseCountMax,

    // Invite chronological constraints
    #[strum(serialize = "project_invites_expires_in_future")]
    ExpiresInFuture,
    #[strum(serialize = "project_invites_expires_after_created")]
    ExpiresAfterCreated,
    #[strum(serialize = "project_invites_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "project_invites_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "project_invites_accepted_after_created")]
    AcceptedAfterCreated,

    // Invite business logic constraints
    #[strum(serialize = "project_invites_accept_status_consistency")]
    AcceptStatusConsistency,
    #[strum(serialize = "project_invites_acceptor_consistency")]
    AcceptorConsistency,

    // Invite unique constraints
    #[strum(serialize = "project_invites_token_unique_idx")]
    TokenUnique,
}

impl From<ProjectInviteConstraints> for String {
    #[inline]
    fn from(val: ProjectInviteConstraints) -> Self {
        val.to_string()
    }
}

impl ProjectInviteConstraints {
    /// Creates a new [`ProjectInviteConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            ProjectInviteConstraints::EmailValid
            | ProjectInviteConstraints::InviteMessageLengthMax
            | ProjectInviteConstraints::InviteTokenNotEmpty
            | ProjectInviteConstraints::StatusReasonLengthMax
            | ProjectInviteConstraints::MaxUsesMin
            | ProjectInviteConstraints::MaxUsesMax
            | ProjectInviteConstraints::UseCountMin
            | ProjectInviteConstraints::UseCountMax => ConstraintCategory::Validation,

            ProjectInviteConstraints::ExpiresInFuture
            | ProjectInviteConstraints::ExpiresAfterCreated
            | ProjectInviteConstraints::UpdatedAfterCreated
            | ProjectInviteConstraints::DeletedAfterUpdated
            | ProjectInviteConstraints::AcceptedAfterCreated => ConstraintCategory::Chronological,

            ProjectInviteConstraints::AcceptStatusConsistency
            | ProjectInviteConstraints::AcceptorConsistency => ConstraintCategory::BusinessLogic,

            ProjectInviteConstraints::TokenUnique => ConstraintCategory::Uniqueness,
        }
    }
}

impl TryFrom<String> for ProjectInviteConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
