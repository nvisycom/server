//! Webhook event emission and delivery services.
//!
//! Provides helpers for emitting domain events to webhooks via NATS JetStream
//! ([`WebhookEmitter`]) and the background worker that delivers them
//! ([`WebhookDeliveryWorker`]).

mod emitter;
mod job;
mod worker;

pub use emitter::WebhookEmitter;
pub use job::WebhookJob;
pub use worker::WebhookDeliveryWorker;
