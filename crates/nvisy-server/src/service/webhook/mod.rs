//! Webhook event emission.
//!
//! The request-side [`WebhookEmitter`] queries the webhooks subscribed to an
//! event and publishes a slim delivery job per webhook onto NATS JetStream. The
//! background worker that delivers them ([`WebhookDeliveryWorker`]) and the job
//! and stream types live in [`worker::webhook`](crate::worker::webhook).

mod emitter;

pub use emitter::WebhookEmitter;
