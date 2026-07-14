//! Account API tokens table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Account API tokens table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountApiTokenConstraints {
    // Token validation constraints
    #[strum(serialize = "account_api_tokens_name_not_empty")]
    NameNotEmpty,
    #[strum(serialize = "account_api_tokens_name_length")]
    NameLength,

    // Token chronological constraints
    #[strum(serialize = "account_api_tokens_expired_after_issued")]
    ExpiredAfterIssued,
    #[strum(serialize = "account_api_tokens_deleted_after_issued")]
    DeletedAfterIssued,
    #[strum(serialize = "account_api_tokens_last_used_after_issued")]
    LastUsedAfterIssued,
}

impl AccountApiTokenConstraints {
    /// Creates a new [`AccountApiTokenConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountApiTokenConstraints::NameNotEmpty | AccountApiTokenConstraints::NameLength => {
                ConstraintCategory::Validation
            }

            AccountApiTokenConstraints::ExpiredAfterIssued
            | AccountApiTokenConstraints::DeletedAfterIssued
            | AccountApiTokenConstraints::LastUsedAfterIssued => ConstraintCategory::Chronological,
        }
    }
}

impl From<AccountApiTokenConstraints> for String {
    #[inline]
    fn from(val: AccountApiTokenConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for AccountApiTokenConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
