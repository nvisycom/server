//! Outbox status enumeration for the event-outbox drainer.

use super::db_enum;

db_enum! {
    /// The processing state of an event-outbox row.
    ///
    /// Corresponds to the `OUTBOX_STATUS` PostgreSQL enum. A row is `Pending`
    /// until the drainer durably projects it (`Processed`) or gives up on it after
    /// too many failed attempts (`Failed`, i.e. dead-lettered).
    pub enum OutboxStatus: Default = Pending, "crate::schema::sql_types::OutboxStatus" {
        /// Awaiting projection, or deferred for a later retry.
        Pending = "pending",
        /// Durably projected to its sinks.
        Processed = "processed",
        /// Given up on after too many failed attempts (dead-lettered).
        Failed = "failed",
    }
}
