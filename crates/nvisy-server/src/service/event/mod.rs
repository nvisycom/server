//! Workspace events via a transactional outbox.
//!
//! A workspace action raises one [`WorkspaceEvent`] carrying the domain facts.
//! [`EventEmitter::emit_event`] — a trait on the connection — records it by
//! inserting a single outbox row, so wrapping the action and the emit in one
//! transaction makes them atomic: the event is never lost, nor recorded for an
//! action that rolled back. The [`EventOutboxDrainer`] then projects each pending
//! event onto the sinks that care — the activity log, the webhook stream, and
//! notifications — asynchronously and with retries. The event → sink projection
//! lives in the drainer alone, off the request path.

mod drainer;
mod emitter;
mod workspace_event;

use uuid::Uuid;

use crate::extract::SecurityContext;
pub use crate::service::event::drainer::EventOutboxDrainer;
pub use crate::service::event::emitter::{EventEmitter, event_outbox_row};
pub use crate::service::event::workspace_event::{
    ConnectionRef, FileRef, InviteRef, MemberRef, PipelineRef, PipelineRunRef, PolicyRef,
    WebhookRef, WorkspaceEvent, WorkspaceRef,
};

/// Who raised an event and where.
///
/// Paired with a [`WorkspaceEvent`] and passed to [`EventEmitter::emit_event`],
/// which records both onto the connection the action runs through.
#[derive(Debug, Clone, Copy)]
pub struct EventOrigin<'a> {
    /// The workspace the event occurred in.
    pub workspace_id: Uuid,
    /// The account that performed the action.
    pub account_id: Uuid,
    /// The caller's client IP and user agent, stamped onto the activity-log entry
    /// the drainer later writes. Background-raised events (detection, sync) pass
    /// `&SecurityContext::default()`, whose fields are both absent.
    pub security: &'a SecurityContext,
}
