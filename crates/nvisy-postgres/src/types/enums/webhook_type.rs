//! Webhook type enumeration for categorizing webhook origins.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the origin type of a workspace webhook.
///
/// This enumeration corresponds to the `WEBHOOK_TYPE` PostgreSQL enum and is used
/// to distinguish between user-created webhooks and those created by integrations.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::WebhookType"]
pub enum WebhookType {
    /// Webhook created manually by user
    #[db_rename = "provided"]
    #[serde(rename = "provided")]
    #[default]
    Provided,

    /// Webhook created by an integration
    #[db_rename = "integration"]
    #[serde(rename = "integration")]
    Integration,
}

impl WebhookType {
    /// Returns whether the webhook was created manually by a user.
    #[inline]
    pub fn is_provided(self) -> bool {
        matches!(self, WebhookType::Provided)
    }

    /// Returns whether the webhook was created by an integration.
    #[inline]
    pub fn is_integration(self) -> bool {
        matches!(self, WebhookType::Integration)
    }

    /// Returns whether this webhook type requires an integration ID.
    #[inline]
    pub fn requires_integration_id(self) -> bool {
        matches!(self, WebhookType::Integration)
    }
}
