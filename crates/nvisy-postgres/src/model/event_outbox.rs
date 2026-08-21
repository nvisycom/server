//! Transactional-outbox model for workspace events.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::event_outbox;

/// A pending or processed outbox row: a serialized workspace event awaiting (or
/// past) projection onto its sinks.
///
/// The `event` column is an opaque JSON blob to this layer — a serialized
/// server-side `WorkspaceEvent` — so the ORM stays free of the event vocabulary;
/// the drainer decodes it.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = event_outbox)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct EventOutbox {
    /// Unique outbox row identifier.
    pub id: Uuid,
    /// Workspace the event was raised in.
    pub workspace_id: Uuid,
    /// Account that performed the action.
    pub account_id: Uuid,
    /// The serialized workspace event.
    pub event: serde_json::Value,
    /// Client IP at the time of the action, if captured.
    pub ip_address: Option<IpNet>,
    /// Client user agent at the time of the action, if captured.
    pub user_agent: Option<String>,
    /// When the drainer finished projecting it; `None` while pending.
    pub processed_at: Option<Timestamp>,
    /// Number of delivery attempts the drainer has made.
    pub attempts: i32,
    /// When the event was raised.
    pub created_at: Timestamp,
}

/// A new outbox row, inserted in the same transaction as the action it records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = event_outbox)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewEventOutbox {
    /// Workspace the event was raised in.
    pub workspace_id: Uuid,
    /// Account that performed the action.
    pub account_id: Uuid,
    /// Client IP at the time of the action, if captured.
    pub ip_address: Option<IpNet>,
    /// Client user agent at the time of the action, if captured.
    pub user_agent: Option<String>,
    /// The serialized workspace event.
    pub event: serde_json::Value,
}
