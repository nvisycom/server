//! The detection background pipeline: the drainer that relays committed analysis
//! jobs to the work-queue, and the worker that runs them.
//!
//! The request-side [`DetectionQueue`] enqueues a job (transactionally with the
//! detection) and wakes the [`Coordinator`]; the [`DetectionOutboxDrainer`]
//! relays pending jobs to the detection NATS work-queue, and the
//! [`DetectionWorker`] consumes them, runs the pipeline's policies against the
//! document, stores the encrypted audit, and marks the detection `Complete` (or
//! `Failed`). Each terminal transition is broadcast on the detection's core-NATS
//! status subject (see [`detection_subject`]) for SSE watchers and emitted as a
//! webhook event.
//!
//! [`DetectionQueue`]: crate::service::DetectionQueue
//! [`Coordinator`]: crate::worker::Coordinator

mod drainer;
mod job;
mod support;
mod worker;

pub use drainer::DetectionOutboxDrainer;
pub use job::{
    DetectionJob, DetectionStatusEvent, DetectionStream, broadcast_status, detection_subject,
    enqueue, subscribe_status,
};
pub use worker::DetectionWorker;
