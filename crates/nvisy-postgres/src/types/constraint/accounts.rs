//! Accounts table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Account table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountConstraints {
    #[strum(serialize = "accounts_username_length")]
    UsernameLength,
    #[strum(serialize = "accounts_username_format")]
    UsernameFormat,
    #[strum(serialize = "accounts_display_name_length")]
    DisplayNameLength,
    #[strum(serialize = "accounts_display_name_not_empty")]
    DisplayNameNotEmpty,
    #[strum(serialize = "accounts_email_format")]
    EmailFormat,
    #[strum(serialize = "accounts_email_length_max")]
    EmailLengthMax,
    #[strum(serialize = "accounts_password_hash_not_empty")]
    PasswordHashNotEmpty,
    #[strum(serialize = "accounts_password_hash_length_min")]
    PasswordHashLengthMin,
    #[strum(serialize = "accounts_timezone_format")]
    TimezoneFormat,
    #[strum(serialize = "accounts_locale_format")]
    LocaleFormat,
    #[strum(serialize = "accounts_suspended_not_admin")]
    SuspendedNotAdmin,
    #[strum(serialize = "accounts_username_unique_idx")]
    UsernameUnique,
    #[strum(serialize = "accounts_email_address_unique_idx")]
    EmailUnique,
}

impl AccountConstraints {
    /// Creates a new [`AccountConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }
}

impl From<AccountConstraints> for String {
    #[inline]
    fn from(val: AccountConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for AccountConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
