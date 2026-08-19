//! Account API tokens table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Account API tokens table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountApiTokenConstraints {
    #[strum(serialize = "account_api_tokens_display_name_not_empty")]
    NameNotEmpty,
    #[strum(serialize = "account_api_tokens_display_name_length")]
    NameLength,
}

impl AccountApiTokenConstraints {
    /// Creates a new [`AccountApiTokenConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
