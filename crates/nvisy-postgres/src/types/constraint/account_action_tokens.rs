//! Account action tokens table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Account action tokens table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountActionTokenConstraints {
    // Token primary key constraint
    #[strum(serialize = "account_action_tokens_pkey")]
    PrimaryKey,

    // Token validation constraints
    #[strum(serialize = "account_action_tokens_action_data_size")]
    ActionDataSize,
    #[strum(serialize = "account_action_tokens_user_agent_not_empty")]
    UserAgentNotEmpty,
    #[strum(serialize = "account_action_tokens_attempt_count_range")]
    AttemptCountRange,
    #[strum(serialize = "account_action_tokens_max_attempts_range")]
    MaxAttemptsRange,

    // Token chronological constraints
    #[strum(serialize = "account_action_tokens_expired_after_issued")]
    ExpiredAfterIssued,
    #[strum(serialize = "account_action_tokens_used_after_issued")]
    UsedAfterIssued,
    #[strum(serialize = "account_action_tokens_used_before_expired")]
    UsedBeforeExpired,
}

impl AccountActionTokenConstraints {
    /// Creates a new [`AccountActionTokenConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountActionTokenConstraints::ActionDataSize
            | AccountActionTokenConstraints::UserAgentNotEmpty
            | AccountActionTokenConstraints::AttemptCountRange
            | AccountActionTokenConstraints::MaxAttemptsRange => ConstraintCategory::Validation,

            AccountActionTokenConstraints::ExpiredAfterIssued
            | AccountActionTokenConstraints::UsedAfterIssued
            | AccountActionTokenConstraints::UsedBeforeExpired => ConstraintCategory::Chronological,

            AccountActionTokenConstraints::PrimaryKey => ConstraintCategory::Uniqueness,
        }
    }
}

impl From<AccountActionTokenConstraints> for String {
    #[inline]
    fn from(val: AccountActionTokenConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for AccountActionTokenConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
