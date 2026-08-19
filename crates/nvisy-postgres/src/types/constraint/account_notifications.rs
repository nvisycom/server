//! Account notifications table constraint violations.

use strum::EnumString;

/// Account notifications table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum AccountNotificationConstraints {
    #[strum(serialize = "account_notifications_params_size")]
    ParamsSize,
}
