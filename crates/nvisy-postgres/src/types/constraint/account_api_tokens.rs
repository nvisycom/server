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
    #[strum(serialize = "account_api_tokens_description_length")]
    DescriptionLength,
    #[strum(serialize = "account_api_tokens_region_code_valid")]
    RegionCodeValid,
    #[strum(serialize = "account_api_tokens_country_code_valid")]
    CountryCodeValid,

    // Token chronological constraints
    #[strum(serialize = "account_api_tokens_expired_after_issued")]
    ExpiredAfterIssued,
    #[strum(serialize = "account_api_tokens_deleted_after_issued")]
    DeletedAfterIssued,
    #[strum(serialize = "account_api_tokens_last_used_after_issued")]
    LastUsedAfterIssued,

    // Token unique constraints
    #[strum(serialize = "account_api_tokens_access_seq_unique_idx")]
    AccessSeqUnique,
    #[strum(serialize = "account_api_tokens_refresh_seq_unique_idx")]
    RefreshSeqUnique,
}

impl AccountApiTokenConstraints {
    /// Creates a new [`AccountApiTokenConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountApiTokenConstraints::NameNotEmpty
            | AccountApiTokenConstraints::NameLength
            | AccountApiTokenConstraints::DescriptionLength
            | AccountApiTokenConstraints::RegionCodeValid
            | AccountApiTokenConstraints::CountryCodeValid => ConstraintCategory::Validation,

            AccountApiTokenConstraints::ExpiredAfterIssued
            | AccountApiTokenConstraints::DeletedAfterIssued
            | AccountApiTokenConstraints::LastUsedAfterIssued => ConstraintCategory::Chronological,

            AccountApiTokenConstraints::AccessSeqUnique
            | AccountApiTokenConstraints::RefreshSeqUnique => ConstraintCategory::Uniqueness,
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
