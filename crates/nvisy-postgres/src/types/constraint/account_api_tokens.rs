//! Account API tokens table constraint violations.

use strum::EnumString;

/// Account API tokens table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum AccountApiTokenConstraints {
    #[strum(serialize = "account_api_tokens_display_name_not_empty")]
    NameNotEmpty,
    #[strum(serialize = "account_api_tokens_display_name_length")]
    NameLength,
}
