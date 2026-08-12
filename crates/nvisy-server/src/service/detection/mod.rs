//! Async pipeline detection.
//!
//! A pipeline run is created synchronously by the API, then its detection
//! (analyze) is enqueued to the `DetectionStream` work-queue and handled by the
//! [`DetectionWorker`] off the request thread. Status changes are broadcast on a
//! core-NATS subject (see [`run_subject`]) for SSE watchers and emitted as
//! webhook events.

mod job;
mod service;
mod support;
mod worker;

pub use job::{DetectionJob, RunStatusEvent, run_subject};
pub use service::DetectionQueue;
pub(crate) use support::{fail_run, resolve_policies};
pub use worker::DetectionWorker;
