//! Account sessions table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::ConstraintCategory;

/// Account sessions table constraint violations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumString, Display, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum AccountSessionConstraints {
    // Session validation constraints
    #[strum(serialize = "account_sessions_region_code_valid")]
    RegionCodeValid,
    #[strum(serialize = "account_sessions_country_code_valid")]
    CountryCodeValid,
    #[strum(serialize = "account_sessions_user_agent_not_empty")]
    UserAgentNotEmpty,

    // Session chronological constraints
    #[strum(serialize = "account_sessions_expired_after_issued")]
    ExpiredAfterIssued,
    #[strum(serialize = "account_sessions_deleted_after_issued")]
    DeletedAfterIssued,
    #[strum(serialize = "account_sessions_last_used_after_issued")]
    LastUsedAfterIssued,

    // Session unique constraints
    #[strum(serialize = "account_sessions_access_seq_unique_idx")]
    AccessSeqUnique,
    #[strum(serialize = "account_sessions_refresh_seq_unique_idx")]
    RefreshSeqUnique,
}

impl AccountSessionConstraints {
    /// Creates a new [`AccountSessionConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountSessionConstraints::RegionCodeValid
            | AccountSessionConstraints::CountryCodeValid
            | AccountSessionConstraints::UserAgentNotEmpty => ConstraintCategory::Validation,

            AccountSessionConstraints::ExpiredAfterIssued
            | AccountSessionConstraints::DeletedAfterIssued
            | AccountSessionConstraints::LastUsedAfterIssued => ConstraintCategory::Chronological,

            AccountSessionConstraints::AccessSeqUnique
            | AccountSessionConstraints::RefreshSeqUnique => ConstraintCategory::Uniqueness,
        }
    }
}

impl From<AccountSessionConstraints> for String {
    #[inline]
    fn from(val: AccountSessionConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for AccountSessionConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
