//! Webhook status enumeration for webhook lifecycle management.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the operational status of a project webhook.
///
/// This enumeration corresponds to the `WEBHOOK_STATUS` PostgreSQL enum and is used
/// to manage webhook states from active operation through pausing and disabling.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::WebhookStatus"]
pub enum WebhookStatus {
    /// Webhook is active and will receive events
    #[db_rename = "active"]
    #[serde(rename = "active")]
    #[default]
    Active,

    /// Webhook is temporarily paused
    #[db_rename = "paused"]
    #[serde(rename = "paused")]
    Paused,

    /// Webhook is disabled (e.g., too many failures)
    #[db_rename = "disabled"]
    #[serde(rename = "disabled")]
    Disabled,
}

impl WebhookStatus {
    /// Returns whether the webhook is active and receiving events.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(self, WebhookStatus::Active)
    }

    /// Returns whether the webhook is paused.
    #[inline]
    pub fn is_paused(self) -> bool {
        matches!(self, WebhookStatus::Paused)
    }

    /// Returns whether the webhook is disabled.
    #[inline]
    pub fn is_disabled(self) -> bool {
        matches!(self, WebhookStatus::Disabled)
    }

    /// Returns whether the webhook can be activated.
    #[inline]
    pub fn can_activate(self) -> bool {
        matches!(self, WebhookStatus::Paused | WebhookStatus::Disabled)
    }
}
