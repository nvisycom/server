//! Webhook status enumeration for webhook lifecycle management.

use super::db_enum;

db_enum! {
    /// The operational status of a workspace webhook.
    ///
    /// Corresponds to the `WEBHOOK_STATUS` PostgreSQL enum. The user controls
    /// `Enabled` / `Disabled`; `Suspended` is set by the system when a webhook
    /// fails repeatedly, and the user can re-enable it.
    pub enum WebhookStatus: Default = Enabled, "crate::schema::sql_types::WebhookStatus" {
        /// Webhook is enabled and will receive events.
        Enabled = "enabled",
        /// Webhook was disabled by the user.
        Disabled = "disabled",
        /// Webhook was suspended by the system (e.g. too many failures).
        Suspended = "suspended",
    }
}

impl WebhookStatus {
    /// Returns whether the webhook is enabled and receiving events.
    #[inline]
    pub fn is_enabled(self) -> bool {
        matches!(self, WebhookStatus::Enabled)
    }

    /// Returns whether the webhook was suspended by the system.
    #[inline]
    pub fn is_suspended(self) -> bool {
        matches!(self, WebhookStatus::Suspended)
    }
}
