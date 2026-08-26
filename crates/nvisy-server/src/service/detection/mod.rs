//! Async pipeline detection.
//!
//! A detection is created synchronously by the API, then its analysis is enqueued
//! to the `DetectionStream` work-queue and handled by the [`DetectionWorker`] off
//! the request thread. Status changes are broadcast on a core-NATS subject (see
//! [`detection_subject`]) for SSE watchers and emitted as webhook events.

mod job;
mod service;
mod support;
mod worker;

pub use job::{DetectionJob, DetectionStatusEvent, detection_subject};
pub use service::DetectionQueue;
pub(crate) use support::{FailDetection, fail_detection, resolve_policies};
pub use worker::DetectionWorker;
