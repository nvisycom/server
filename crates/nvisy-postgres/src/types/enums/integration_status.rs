//! Integration status enumeration for integration lifecycle management.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the operational status of a project integration.
///
/// This enumeration corresponds to the `INTEGRATION_STATUS` PostgreSQL enum and is used
/// to manage integration states from initial setup through active execution and error handling.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::IntegrationStatus"]
pub enum IntegrationStatus {
    /// Integration is pending configuration or activation
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Integration is actively executing and operational
    #[db_rename = "executing"]
    #[serde(rename = "executing")]
    Executing,

    /// Integration has encountered an error or failure
    #[db_rename = "failure"]
    #[serde(rename = "failure")]
    Failure,
}

impl IntegrationStatus {
    /// Returns whether the integration is operational.
    #[inline]
    pub fn is_operational(self) -> bool {
        matches!(self, IntegrationStatus::Executing)
    }

    /// Returns whether the integration is pending setup.
    #[inline]
    pub fn is_pending(self) -> bool {
        matches!(self, IntegrationStatus::Pending)
    }

    /// Returns whether the integration has failed.
    #[inline]
    pub fn has_failed(self) -> bool {
        matches!(self, IntegrationStatus::Failure)
    }

    /// Returns whether the integration can be activated.
    #[inline]
    pub fn can_activate(self) -> bool {
        matches!(
            self,
            IntegrationStatus::Pending | IntegrationStatus::Failure
        )
    }

    /// Returns whether the integration can be configured.
    #[inline]
    pub fn can_configure(self) -> bool {
        // All statuses allow configuration changes
        true
    }

    /// Returns a description of what the integration status means.
    #[inline]
    pub fn description(self) -> &'static str {
        match self {
            IntegrationStatus::Pending => "Integration is awaiting configuration or activation",
            IntegrationStatus::Executing => "Integration is actively running and operational",
            IntegrationStatus::Failure => {
                "Integration has encountered an error and requires attention"
            }
        }
    }
}
