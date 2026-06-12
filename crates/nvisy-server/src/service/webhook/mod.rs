//! Webhook event emission and delivery services.
//!
//! Provides helpers for emitting domain events to webhooks via NATS JetStream
//! ([`WebhookEmitter`]) and the background worker that delivers them
//! ([`WebhookWorker`]).

mod emitter;
mod worker;

pub use emitter::WebhookEmitter;
pub use worker::WebhookWorker;
