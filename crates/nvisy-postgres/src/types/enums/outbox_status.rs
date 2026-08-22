//! Outbox status enumeration for the event-outbox drainer.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// The processing state of an event-outbox row.
///
/// Corresponds to the `OUTBOX_STATUS` PostgreSQL enum. A row is `Pending` until
/// the drainer durably projects it (`Processed`) or gives up on it after too many
/// failed attempts (`Failed`, i.e. dead-lettered).
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::OutboxStatus"]
pub enum OutboxStatus {
    /// Awaiting projection, or deferred for a later retry.
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Durably projected to its sinks.
    #[db_rename = "processed"]
    #[serde(rename = "processed")]
    Processed,

    /// Given up on after too many failed attempts (dead-lettered).
    #[db_rename = "failed"]
    #[serde(rename = "failed")]
    Failed,
}
