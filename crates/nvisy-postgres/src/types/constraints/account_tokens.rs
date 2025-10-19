//! Account tokens table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Account tokens table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountTokenConstraints {
    // Token primary key constraint
    #[strum(serialize = "account_tokens_pkey")]
    PrimaryKey,

    // Token validation constraints
    #[strum(serialize = "account_tokens_action_data_size_min")]
    ActionDataSizeMin,
    #[strum(serialize = "account_tokens_action_data_size_max")]
    ActionDataSizeMax,
    #[strum(serialize = "account_tokens_user_agent_not_empty")]
    UserAgentNotEmpty,
    #[strum(serialize = "account_tokens_attempt_count_min")]
    AttemptCountMin,
    #[strum(serialize = "account_tokens_attempt_count_max")]
    AttemptCountMax,
    #[strum(serialize = "account_tokens_max_attempts_min")]
    MaxAttemptsMin,
    #[strum(serialize = "account_tokens_max_attempts_max")]
    MaxAttemptsMax,

    // Token chronological constraints
    #[strum(serialize = "account_tokens_expired_after_issued")]
    ExpiredAfterIssued,
    #[strum(serialize = "account_tokens_used_after_issued")]
    UsedAfterIssued,
    #[strum(serialize = "account_tokens_used_before_expired")]
    UsedBeforeExpired,
}

impl AccountTokenConstraints {
    /// Creates a new [`AccountTokenConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountTokenConstraints::ActionDataSizeMin
            | AccountTokenConstraints::ActionDataSizeMax
            | AccountTokenConstraints::UserAgentNotEmpty
            | AccountTokenConstraints::AttemptCountMin
            | AccountTokenConstraints::AttemptCountMax
            | AccountTokenConstraints::MaxAttemptsMin
            | AccountTokenConstraints::MaxAttemptsMax => ConstraintCategory::Validation,

            AccountTokenConstraints::ExpiredAfterIssued
            | AccountTokenConstraints::UsedAfterIssued
            | AccountTokenConstraints::UsedBeforeExpired => ConstraintCategory::Chronological,

            AccountTokenConstraints::PrimaryKey => ConstraintCategory::Uniqueness,
        }
    }
}

impl From<AccountTokenConstraints> for String {
    #[inline]
    fn from(val: AccountTokenConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for AccountTokenConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
