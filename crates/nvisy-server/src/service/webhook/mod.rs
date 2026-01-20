//! Webhook event emission service.
//!
//! Provides helpers for emitting domain events to webhooks via NATS JetStream.

mod emitter;

pub use emitter::WebhookEmitter;
