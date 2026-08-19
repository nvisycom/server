//! Accounts table constraint violations.

use strum::EnumString;

/// Account table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
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
