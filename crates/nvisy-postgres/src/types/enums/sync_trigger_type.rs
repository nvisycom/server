//! Sync trigger type enumeration: whether a run was scheduled or on-demand.

use super::db_enum;

db_enum! {
    /// How a connection sync run was initiated.
    ///
    /// Corresponds to the `SYNC_TRIGGER_TYPE` PostgreSQL enum. The axis is who
    /// initiated the run: the connection's own schedule, or anything else — a
    /// user, the SDK, an external automation (Zapier and the like). The latter is
    /// `OnDemand`, which covers every non-scheduled trigger, not only a human
    /// click.
    pub enum SyncTriggerType: Default = OnDemand, "crate::schema::sql_types::SyncTriggerType" {
        /// Triggered outside the schedule: a user, the SDK, or an external
        /// automation.
        OnDemand = "on_demand",
        /// Triggered by the connection's schedule.
        Scheduled = "scheduled",
    }
}
