//! Integration type enumeration for categorizing project integrations.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type/category of a project integration.
///
/// This enumeration corresponds to the `INTEGRATION_TYPE` PostgreSQL enum and is used
/// to categorize different types of third-party integrations that can be connected to projects.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::IntegrationType"]
pub enum IntegrationType {
    /// Generic webhook integration
    #[db_rename = "webhook"]
    #[serde(rename = "webhook")]
    Webhook,

    /// External storage integration (S3, etc.)
    #[db_rename = "storage"]
    #[serde(rename = "storage")]
    Storage,

    /// Other integration types
    #[db_rename = "other"]
    #[serde(rename = "other")]
    #[default]
    Other,
}

impl IntegrationType {
    /// Returns whether this is a webhook integration.
    #[inline]
    pub fn is_webhook(self) -> bool {
        matches!(self, IntegrationType::Webhook)
    }

    /// Returns whether this is a storage integration.
    #[inline]
    pub fn is_storage(self) -> bool {
        matches!(self, IntegrationType::Storage)
    }

    /// Returns whether this integration type supports webhooks.
    #[inline]
    pub fn supports_webhooks(self) -> bool {
        matches!(self, IntegrationType::Webhook)
    }

    /// Returns whether this integration type requires credentials.
    #[inline]
    pub fn requires_credentials(self) -> bool {
        matches!(self, IntegrationType::Storage)
    }
}
