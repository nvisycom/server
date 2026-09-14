//! The request-side handle for the assistant subsystem.

use crate::response::Result;
use crate::service::Infra;
use crate::worker::Coordinator;
use crate::worker::assistant::{AssistantJob, AssistantStream};

/// Enqueues assistant-reply jobs onto the work-queue.
///
/// Cheaply cloneable (holds the shared [`Infra`] clients and the [`Coordinator`],
/// all `Arc`-backed).
#[derive(Clone)]
#[must_use = "service does nothing unless you enqueue with it"]
pub struct AssistantQueue {
    infra: Infra,
    coordinator: Coordinator,
}

impl AssistantQueue {
    /// Creates a new [`AssistantQueue`].
    pub fn new(infra: Infra, coordinator: Coordinator) -> Self {
        Self { infra, coordinator }
    }

    /// Enqueues an assistant reply onto the work-queue for the worker to pick up.
    ///
    /// # Errors
    ///
    /// - A messaging error if publishing the job to NATS fails.
    pub async fn enqueue(&self, job: AssistantJob) -> Result<()> {
        let publisher = self.infra.nats.event_publisher::<AssistantStream>();
        publisher.publish(&job).await?;
        Ok(())
    }

    /// Wakes the assistant-job outbox drainer so a just-committed job is drained
    /// immediately instead of waiting for the drainer's next timer tick. Call after
    /// the transaction that inserted the job row has committed. Best-effort and
    /// coalescing: the drainer's timer still covers a missed wake (a crash between
    /// commit and this call, or a job that landed on another instance).
    pub fn wake_drainer(&self) {
        self.coordinator.wake();
    }
}
