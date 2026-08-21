//! The event-emit trait: records that a workspace event happened by inserting
//! one row into the transactional outbox.
//!
//! [`EventEmitter`] is a server-side extension trait on the connection, layered
//! over the postgres [`EventOutboxRepository`]: it turns a [`WorkspaceEvent`] plus
//! its origin into one outbox row. Because it writes through the same connection
//! as the action, wrapping the action and the emit in one transaction makes the
//! two atomic — the event can never be lost, nor recorded for an action that
//! rolled back. Emission does no projection; the [drainer](super::drainer) fans
//! each event out to the activity log, webhook stream, and notifications later.

use std::future::Future;

use nvisy_postgres::PgConn;
use nvisy_postgres::model::NewEventOutbox;
use nvisy_postgres::query::EventOutboxRepository;

use crate::handler::{ErrorKind, Result};
use crate::service::event::{EventOrigin, WorkspaceEvent};

/// Builds the outbox row for an event and its origin.
///
/// Serialization is the only fallible step, so callers that must insert the row
/// inside a `PgError`-typed transaction (to preserve a rollback sentinel) can
/// build the row up front with this and then insert it via
/// [`EventOutboxRepository::insert_event_outbox`].
pub fn event_outbox_row(origin: EventOrigin<'_>, event: &WorkspaceEvent) -> Result<NewEventOutbox> {
    let event = serde_json::to_value(event).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to serialize workspace event")
            .with_context(err.to_string())
    })?;
    Ok(NewEventOutbox {
        workspace_id: origin.workspace_id,
        account_id: origin.account_id,
        ip_address: origin.security.ip_address,
        user_agent: origin.security.user_agent.clone(),
        event,
    })
}

/// Records workspace events into the transactional outbox, on the connection the
/// action runs through.
pub trait EventEmitter {
    /// Records `event` by inserting one outbox row.
    ///
    /// Call this inside the action's transaction (`conn.emit_event(..).await?`)
    /// so a failed insert rolls the action back, keeping the event and the action
    /// atomic. The event's projection to its sinks happens later, in the drainer.
    fn emit_event(
        &mut self,
        origin: EventOrigin<'_>,
        event: WorkspaceEvent,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl EventEmitter for PgConn {
    async fn emit_event(&mut self, origin: EventOrigin<'_>, event: WorkspaceEvent) -> Result<()> {
        let row = event_outbox_row(origin, &event)?;
        self.insert_event_outbox(row).await?;
        Ok(())
    }
}
