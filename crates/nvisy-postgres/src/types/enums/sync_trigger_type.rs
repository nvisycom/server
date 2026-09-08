//! Sync trigger type enumeration: whether a run was scheduled or on-demand.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines how a connection sync run was initiated.
///
/// Corresponds to the `SYNC_TRIGGER_TYPE` PostgreSQL enum. The axis is who
/// initiated the run: the connection's own schedule, or anything else — a user,
/// the SDK, an external automation (Zapier and the like). The latter is
/// `OnDemand`, which covers every non-scheduled trigger, not only a human click.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::SyncTriggerType"]
pub enum SyncTriggerType {
    /// Triggered outside the schedule: a user, the SDK, or an external
    /// automation.
    #[db_rename = "on_demand"]
    #[serde(rename = "on_demand")]
    #[default]
    OnDemand,

    /// Triggered by the connection's schedule.
    #[db_rename = "scheduled"]
    #[serde(rename = "scheduled")]
    Scheduled,
}
