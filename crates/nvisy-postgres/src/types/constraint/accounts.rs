//! Accounts table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Account table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountConstraints {
    // Account validation constraints
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

    // Account uniqueness constraints
    #[strum(serialize = "accounts_username_unique_idx")]
    UsernameUnique,

    // Account chronological constraints
    #[strum(serialize = "accounts_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "accounts_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "accounts_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "accounts_password_changed_after_created")]
    PasswordChangedAfterCreated,
}

impl AccountConstraints {
    /// Creates a new [`AccountConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountConstraints::UsernameLength
            | AccountConstraints::UsernameFormat
            | AccountConstraints::DisplayNameLength
            | AccountConstraints::DisplayNameNotEmpty
            | AccountConstraints::EmailFormat
            | AccountConstraints::EmailLengthMax
            | AccountConstraints::PasswordHashNotEmpty
            | AccountConstraints::PasswordHashLengthMin
            | AccountConstraints::TimezoneFormat
            | AccountConstraints::LocaleFormat
            | AccountConstraints::SuspendedNotAdmin => ConstraintCategory::Validation,

            AccountConstraints::UsernameUnique => ConstraintCategory::Uniqueness,

            AccountConstraints::UpdatedAfterCreated
            | AccountConstraints::DeletedAfterCreated
            | AccountConstraints::DeletedAfterUpdated
            | AccountConstraints::PasswordChangedAfterCreated => ConstraintCategory::Chronological,
        }
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
