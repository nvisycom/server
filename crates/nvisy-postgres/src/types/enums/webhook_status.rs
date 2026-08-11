//! Webhook status enumeration for webhook lifecycle management.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the operational status of a workspace webhook.
///
/// This enumeration corresponds to the `WEBHOOK_STATUS` PostgreSQL enum. The
/// user controls `Enabled` / `Disabled`; `Suspended` is set by the system when a
/// webhook fails repeatedly, and the user can re-enable it.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::WebhookStatus"]
pub enum WebhookStatus {
    /// Webhook is enabled and will receive events.
    #[db_rename = "enabled"]
    #[serde(rename = "enabled")]
    #[default]
    Enabled,

    /// Webhook was disabled by the user.
    #[db_rename = "disabled"]
    #[serde(rename = "disabled")]
    Disabled,

    /// Webhook was suspended by the system (e.g., too many failures).
    #[db_rename = "suspended"]
    #[serde(rename = "suspended")]
    Suspended,
}

impl WebhookStatus {
    /// Returns whether the webhook is enabled and receiving events.
    #[inline]
    pub fn is_enabled(self) -> bool {
        matches!(self, WebhookStatus::Enabled)
    }

    /// Returns whether the webhook was disabled by the user.
    #[inline]
    pub fn is_disabled(self) -> bool {
        matches!(self, WebhookStatus::Disabled)
    }

    /// Returns whether the webhook was suspended by the system.
    #[inline]
    pub fn is_suspended(self) -> bool {
        matches!(self, WebhookStatus::Suspended)
    }

    /// Returns whether the webhook can be re-enabled (it is off, whether by the
    /// user or the system).
    #[inline]
    pub fn can_enable(self) -> bool {
        matches!(self, WebhookStatus::Disabled | WebhookStatus::Suspended)
    }
}
