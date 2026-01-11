//! Integration status enumeration for integration lifecycle management.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the operational status of a workspace integration.
///
/// This enumeration corresponds to the `INTEGRATION_STATUS` PostgreSQL enum and is used
/// to manage integration states from initial setup through active execution and cancellation.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::IntegrationStatus"]
pub enum IntegrationStatus {
    /// Integration is pending configuration or activation
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Integration is actively running and operational
    #[db_rename = "running"]
    #[serde(rename = "running")]
    Running,

    /// Integration has been cancelled
    #[db_rename = "cancelled"]
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl IntegrationStatus {
    /// Returns whether the integration is operational.
    #[inline]
    pub fn is_operational(self) -> bool {
        matches!(self, IntegrationStatus::Running)
    }

    /// Returns whether the integration is running.
    #[inline]
    pub fn is_running(self) -> bool {
        matches!(self, IntegrationStatus::Running)
    }

    /// Returns whether the integration is pending setup.
    #[inline]
    pub fn is_pending(self) -> bool {
        matches!(self, IntegrationStatus::Pending)
    }

    /// Returns whether the integration has been cancelled.
    #[inline]
    pub fn is_cancelled(self) -> bool {
        matches!(self, IntegrationStatus::Cancelled)
    }

    /// Returns whether the integration can be activated.
    #[inline]
    pub fn can_activate(self) -> bool {
        matches!(
            self,
            IntegrationStatus::Pending | IntegrationStatus::Cancelled
        )
    }

    /// Returns whether the integration can be configured.
    #[inline]
    pub fn can_configure(self) -> bool {
        // All statuses allow configuration changes
        true
    }
}
