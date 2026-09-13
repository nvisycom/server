//! Event-projection sinks.
//!
//! The event outbox drainer projects each workspace event onto the sinks that
//! care; these are the request-independent emit handles it fans out to. Each is a
//! cheap [`Infra`](crate::service::Infra)-backed handle composed on demand, the
//! background-side counterpart to the request-time enqueue handles in
//! [`queue`](crate::service::queue).
//!
//! - [`WebhookEmitter`] publishes one delivery job per subscribed webhook onto
//!   the webhook work-queue (consumed by
//!   [`worker::webhook`](crate::worker::webhook)).
//! - [`NotificationEmitter`] writes in-app notifications and broadcasts unread
//!   counts.

mod notification;
mod webhook;

pub use notification::{NotificationEmitter, UnreadCountEvent};
pub use webhook::WebhookEmitter;
