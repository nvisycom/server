//! Sync trigger type enumeration indicating how a connection sync run was initiated.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines how a connection sync run was initiated.
///
/// This enumeration corresponds to the `SYNC_TRIGGER_TYPE` PostgreSQL enum and is used
/// to track whether a run was manually triggered, scheduled, or triggered by a webhook.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::SyncTriggerType"]
pub enum SyncTriggerType {
    /// Manually triggered by user
    #[db_rename = "manual"]
    #[serde(rename = "manual")]
    #[default]
    Manual,

    /// Triggered by schedule
    #[db_rename = "scheduled"]
    #[serde(rename = "scheduled")]
    Scheduled,

    /// Triggered by an inbound webhook
    #[db_rename = "webhook"]
    #[serde(rename = "webhook")]
    Webhook,
}
