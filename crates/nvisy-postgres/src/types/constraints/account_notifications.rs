//! Account notifications table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Account notifications table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountNotificationConstraints {
    // Notification validation constraints
    #[strum(serialize = "account_notifications_title_length")]
    TitleLength,
    #[strum(serialize = "account_notifications_message_length")]
    MessageLength,
    #[strum(serialize = "account_notifications_related_type_length")]
    RelatedTypeLength,
    #[strum(serialize = "account_notifications_metadata_size")]
    MetadataSize,

    // Notification chronological constraints
    #[strum(serialize = "account_notifications_expires_after_created")]
    ExpiresAfterCreated,
    #[strum(serialize = "account_notifications_read_after_created")]
    ReadAfterCreated,
}

impl AccountNotificationConstraints {
    /// Creates a new [`AccountNotificationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            AccountNotificationConstraints::TitleLength
            | AccountNotificationConstraints::MessageLength
            | AccountNotificationConstraints::RelatedTypeLength
            | AccountNotificationConstraints::MetadataSize => ConstraintCategory::Validation,

            AccountNotificationConstraints::ExpiresAfterCreated
            | AccountNotificationConstraints::ReadAfterCreated => ConstraintCategory::Chronological,
        }
    }
}

impl From<AccountNotificationConstraints> for String {
    #[inline]
    fn from(val: AccountNotificationConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for AccountNotificationConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
