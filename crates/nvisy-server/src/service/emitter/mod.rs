//! Event-projection sinks.
//!
//! The event outbox drainer projects each workspace event onto the sinks that
//! care; these are the request-independent emit handles it fans out to. Each is a
//! cheap [`Infra`]-backed handle composed on demand, the background-side
//! counterpart to the request-time enqueue handles in [`queue`].
//!
//! - [`WebhookEmitter`] publishes one delivery job per subscribed webhook onto
//!   the webhook work-queue (consumed by [`worker::webhook`]).
//! - [`NotificationEmitter`] writes in-app notifications and broadcasts unread
//!   counts.
//!
//! [`Infra`]: crate::service::Infra
//! [`queue`]: crate::service::queue
//! [`worker::webhook`]: crate::worker::webhook

mod notification;
mod webhook;

pub use notification::{NotificationEmitter, UnreadCountEvent};
pub use webhook::WebhookEmitter;
