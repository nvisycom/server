//! Assistant enqueue service.
//!
//! The request-side counterpart to the [`AssistantWorker`](super::AssistantWorker):
//! publishes an assistant-reply job to the `AssistantStream` work-queue. Injected
//! into the comment handler so the handler stays thin and the NATS wiring lives in
//! one place.

use super::coordinator::AssistantCoordinator;
use super::job::{AssistantJob, AssistantStream};
use crate::response::Result;
use crate::service::Infra;

/// Enqueues assistant-reply jobs onto the work-queue.
///
/// Cheaply cloneable (holds the shared [`Infra`] clients and the
/// [`AssistantCoordinator`], all `Arc`-backed).
#[derive(Clone)]
#[must_use = "service does nothing unless you enqueue with it"]
pub struct AssistantQueue {
    infra: Infra,
    coordinator: AssistantCoordinator,
}

impl AssistantQueue {
    /// Creates a new [`AssistantQueue`].
    pub fn new(infra: Infra, coordinator: AssistantCoordinator) -> Self {
        Self { infra, coordinator }
    }

    /// Enqueues an assistant reply onto the work-queue for the worker to pick up.
    pub async fn enqueue(&self, job: AssistantJob) -> Result<()> {
        let publisher = self.infra.nats.event_publisher::<AssistantStream>().await?;
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
