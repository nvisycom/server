//! Webhook delivery worker.
//!
//! The background executor that delivers queued webhook jobs
//! ([`WebhookDeliveryWorker`]), plus the job and stream types it consumes. The
//! request-side emitter that enqueues these jobs
//! ([`WebhookEmitter`](crate::service::WebhookEmitter)) stays in `service`.

mod job;
mod worker;

pub use job::{WebhookJob, WebhookStream};
pub use worker::WebhookDeliveryWorker;
