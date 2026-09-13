//! Workspace-event outbox drainer.
//!
//! The background executor ([`EventOutboxDrainer`]) that projects each pending
//! workspace-event outbox row onto its sinks — the activity log, the webhook
//! stream, and notifications — asynchronously and with retries. The request-side
//! emit API (the [`EventEmitter`](crate::service::event::EventEmitter) trait and
//! the [`WorkspaceEvent`](crate::service::event::WorkspaceEvent) types) stays in
//! [`service::event`](crate::service::event).

mod drainer;

pub use drainer::EventOutboxDrainer;
