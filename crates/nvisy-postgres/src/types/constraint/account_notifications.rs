//! Account notifications table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Account notifications table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum AccountNotificationConstraints {
    #[strum(serialize = "account_notifications_params_size")]
    ParamsSize,
}

impl AccountNotificationConstraints {
    /// Creates a new [`AccountNotificationConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
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
