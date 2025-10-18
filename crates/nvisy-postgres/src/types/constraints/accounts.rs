//! Accounts table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

use super::ConstraintCategory;

/// Account table constraint violations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumString, Display, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum AccountConstraints {
    // Account validation constraints
    #[strum(serialize = "accounts_display_name_length_min")]
    DisplayNameLengthMin,
    #[strum(serialize = "accounts_display_name_length_max")]
    DisplayNameLengthMax,
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
    #[strum(serialize = "accounts_company_name_length_max")]
    CompanyNameLengthMax,
    #[strum(serialize = "accounts_phone_number_length_max")]
    PhoneNumberLengthMax,
    #[strum(serialize = "accounts_timezone_format")]
    TimezoneFormat,
    #[strum(serialize = "accounts_locale_format")]
    LocaleFormat,

    // Account security constraints
    #[strum(serialize = "accounts_failed_login_attempts_min")]
    FailedLoginAttemptsMin,
    #[strum(serialize = "accounts_failed_login_attempts_max")]
    FailedLoginAttemptsMax,
    #[strum(serialize = "accounts_locked_until_future")]
    LockedUntilFuture,

    // Account chronological constraints
    #[strum(serialize = "accounts_updated_after_created")]
    UpdatedAfterCreated,
    #[strum(serialize = "accounts_deleted_after_created")]
    DeletedAfterCreated,
    #[strum(serialize = "accounts_deleted_after_updated")]
    DeletedAfterUpdated,
    #[strum(serialize = "accounts_password_changed_after_created")]
    PasswordChangedAfterCreated,
    #[strum(serialize = "accounts_last_login_after_created")]
    LastLoginAfterCreated,

    // Account business logic constraints
    #[strum(serialize = "accounts_suspended_not_admin")]
    SuspendedNotAdmin,

    // Account unique constraints
    #[strum(serialize = "accounts_email_address_unique_idx")]
    EmailAddressUnique,
}

impl AccountConstraints {
    /// Creates a new [`AccountConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountConstraints::DisplayNameLengthMin
            | AccountConstraints::DisplayNameLengthMax
            | AccountConstraints::DisplayNameNotEmpty
            | AccountConstraints::EmailFormat
            | AccountConstraints::EmailLengthMax
            | AccountConstraints::PasswordHashNotEmpty
            | AccountConstraints::PasswordHashLengthMin
            | AccountConstraints::CompanyNameLengthMax
            | AccountConstraints::PhoneNumberLengthMax
            | AccountConstraints::TimezoneFormat
            | AccountConstraints::LocaleFormat
            | AccountConstraints::FailedLoginAttemptsMin
            | AccountConstraints::FailedLoginAttemptsMax => ConstraintCategory::Validation,

            AccountConstraints::UpdatedAfterCreated
            | AccountConstraints::DeletedAfterCreated
            | AccountConstraints::DeletedAfterUpdated
            | AccountConstraints::PasswordChangedAfterCreated
            | AccountConstraints::LastLoginAfterCreated
            | AccountConstraints::LockedUntilFuture => ConstraintCategory::Chronological,

            AccountConstraints::SuspendedNotAdmin => ConstraintCategory::BusinessLogic,

            AccountConstraints::EmailAddressUnique => ConstraintCategory::Uniqueness,
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
