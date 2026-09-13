//! The assistant background pipeline: the drainer that relays committed reply
//! jobs to the work-queue, and the worker that answers them.
//!
//! The request-side [`AssistantQueue`](crate::service::AssistantQueue) enqueues a
//! job (transactionally with the comment) and wakes the
//! [`Coordinator`](crate::worker::Coordinator); the [`AssistantOutboxDrainer`]
//! relays pending jobs to the assistant NATS work-queue, and the
//! [`AssistantWorker`] consumes them, runs the workspace's language model, and
//! posts the reply as the reserved assistant account.

mod drainer;
mod job;
mod worker;

pub use drainer::AssistantOutboxDrainer;
pub use job::{AssistantJob, AssistantStream};
pub use worker::AssistantWorker;
