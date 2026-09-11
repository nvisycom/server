//! In-process wake coordination for the assistant pipeline.

use std::sync::Arc;

use tokio::sync::Notify;

/// The rendezvous between the assistant-job enqueue path and the outbox drainer.
///
/// After a handler commits a new assistant-reply job, it calls [`wake`](Self::wake)
/// so the drainer drains the job at once instead of waiting for its next timer
/// tick; the drainer awaits [`notified`](Self::notified) alongside that timer. The
/// wake is in-process only and best-effort — the drainer's timer remains the
/// cross-instance and crash-safety fallback, and the Postgres claim keeps a job
/// single-drained across the fleet — so a missed wake only defers a drain to the
/// next tick, never strands a job.
///
/// A single instance is shared (it is `Arc`-backed, so clones share one
/// [`Notify`]) between the [`AssistantQueue`](super::AssistantQueue) that
/// materialises per request and the long-lived
/// [`AssistantOutboxDrainer`](super::AssistantOutboxDrainer).
#[derive(Clone, Default)]
#[must_use = "the coordinator does nothing unless you wake or await it"]
pub struct AssistantCoordinator {
    wake: Arc<Notify>,
}

impl AssistantCoordinator {
    /// Creates a new coordinator with no pending wake.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wakes the drainer so a just-committed job is drained immediately. Coalescing
    /// and best-effort: a wake with no waiter is remembered for the next
    /// [`notified`](Self::notified), and extra wakes collapse into one.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Waits for the next [`wake`](Self::wake). Awaited by the drainer alongside
    /// its timer tick.
    pub async fn notified(&self) {
        self.wake.notified().await;
    }
}
